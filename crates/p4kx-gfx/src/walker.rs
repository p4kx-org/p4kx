// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod accepted;

use std::collections::{BTreeMap, HashMap};

use crate::asset_origins::{AssetOrigins, FailureOrigin, SpriteOrigins, TimelineOrigins};
use crate::assets::{Assets, TagCount, TagFailure};
use crate::font_info::FontInfo;
use crate::place::read_place;
use crate::shape::RecordBudget;
use crate::tag::{Tag, TagStream};
use crate::text::GlyphBudget;
use crate::timeline::{DisplayList, MAX_FRAME_BYTES, MAX_FRAME_PLACEMENTS, Sprite, Timeline};
use crate::{GfxError, MovieSpan, TagSite};

/// Applies a `PlaceObject` or `RemoveObject` tag to its display list.
fn read_display_tag(
    tag: &Tag<'_>,
    display_list: &mut DisplayList,
    origin: Option<&mut TimelineOrigins>,
) -> Result<(), GfxError> {
    match tag.code {
        4 | 26 | 70 => display_list.place(read_place(tag)?, tag.site(), origin),
        5 | 28 => display_list.remove(tag, origin)?,
        _ => {}
    }
    Ok(())
}

struct TimelineScan<'movie> {
    stream: TagStream<'movie>,
    end: usize,
    timeline: Timeline,
    display_list: DisplayList,
    origin: Option<TimelineOrigins>,
}

impl<'movie> TimelineScan<'movie> {
    fn new(data: &'movie [u8], base: usize, observe: bool) -> Self {
        Self {
            stream: TagStream::new(data, base),
            end: base + data.len(),
            timeline: Timeline::default(),
            display_list: DisplayList::default(),
            origin: observe.then(|| TimelineOrigins {
                scan: MovieSpan::new(base, base),
                complete: true,
                ..TimelineOrigins::default()
            }),
        }
    }

    fn finish(mut self) -> (Timeline, Option<TimelineOrigins>) {
        self.timeline.finish(self.display_list);
        if let Some(origin) = &mut self.origin {
            origin.scan = MovieSpan::new(origin.scan.offset, self.stream.offset());
        }
        (self.timeline, self.origin)
    }
}

struct SuspendedTimeline<'movie> {
    sprite_index: usize,
    parent: TimelineScan<'movie>,
}

/// Reads every tag of a movie and its sprites into [`Assets`].
pub(crate) struct Walker<'a, 'origin> {
    assets: Assets<'a>,
    counts: BTreeMap<u16, u32>,
    budget: RecordBudget,
    text_budget: GlyphBudget,
    font_infos: Vec<(FontInfo, TagSite)>,
    font_names: Vec<((u16, String), TagSite)>,
    origin: Option<&'origin mut AssetOrigins>,
    unread: BTreeMap<u16, u32>,
    imported: usize,
    frame_placements: usize,
    frame_bytes: usize,
    frames_capped: bool,
}

impl<'a, 'origin> Walker<'a, 'origin> {
    /// Reads the tags of the main timeline in `data`, which starts at `base` in the movie.
    pub(crate) fn read(data: &'a [u8], base: usize) -> Assets<'a> {
        Self::run(data, base, None)
    }

    pub(crate) fn read_observed(data: &'a [u8], base: usize) -> (Assets<'a>, AssetOrigins) {
        let mut origin = AssetOrigins::default();
        let assets = Walker::run(data, base, Some(&mut origin));
        (assets, origin)
    }

    fn run(data: &'a [u8], base: usize, origin: Option<&'origin mut AssetOrigins>) -> Assets<'a> {
        let mut walker = Self {
            assets: Assets::default(),
            counts: BTreeMap::new(),
            budget: RecordBudget::MOVIE,
            text_budget: GlyphBudget::MOVIE,
            font_infos: Vec::new(),
            font_names: Vec::new(),
            origin,
            unread: BTreeMap::new(),
            imported: 0,
            frame_placements: 0,
            frame_bytes: 0,
            frames_capped: false,
        };
        walker.assets.timeline = walker.walk(data, base);
        walker.finish()
    }

    fn finish(mut self) -> Assets<'a> {
        let mut fonts: HashMap<u16, usize> = HashMap::with_capacity(self.assets.fonts.len());
        for (index, font) in self.assets.fonts.iter().enumerate() {
            fonts.entry(font.id).or_insert(index);
        }
        for (info, site) in self.font_infos {
            if let Some(&index) = fonts.get(&info.id)
                && let Some(font) = self.assets.fonts.get_mut(index)
            {
                let codes_written = info.apply(font);
                if let Some(original) = self
                    .origin
                    .as_deref_mut()
                    .and_then(|o| o.fonts.get_mut(index))
                {
                    original.observe_info(site, codes_written);
                }
            }
        }
        for ((id, name), site) in self.font_names {
            if let Some(&index) = fonts.get(&id)
                && let Some(font) = self.assets.fonts.get_mut(index)
            {
                font.full_name = Some(name);
                if let Some(original) = self
                    .origin
                    .as_deref_mut()
                    .and_then(|o| o.fonts.get_mut(index))
                {
                    original.full_name = Some(site);
                }
            }
        }
        for export in &self.assets.exports {
            if let Some(font) = fonts
                .get(&export.id)
                .and_then(|&index| self.assets.fonts.get_mut(index))
                && font.export_name.is_none()
            {
                font.export_name = Some(export.name.clone());
            }
        }
        self.assets.unread_tags = self
            .unread
            .into_iter()
            .map(|(code, count)| TagCount { code, count })
            .collect();
        self.assets.tag_counts = self
            .counts
            .into_iter()
            .map(|(code, count)| TagCount { code, count })
            .collect();
        self.assets
    }

    fn walk(&mut self, data: &'a [u8], base: usize) -> Timeline {
        let mut current = TimelineScan::new(data, base, self.origin.is_some());
        let mut parents: Vec<SuspendedTimeline<'a>> = Vec::new();
        loop {
            let Some(tag) = self.next_tag(&mut current) else {
                let (timeline, observed) = current.finish();
                let Some(suspended) = parents.pop() else {
                    if let (Some(origin), Some(observed)) = (&mut self.origin, observed) {
                        origin.timeline = observed;
                    }
                    return timeline;
                };
                if let (Some(origin), Some(observed)) = (&mut self.origin, observed)
                    && let Some(sprite) = origin.sprites.get_mut(suspended.sprite_index)
                {
                    sprite.timeline = observed;
                }
                if let Some(sprite) = self.assets.sprites.get_mut(suspended.sprite_index) {
                    sprite.timeline = timeline;
                }
                current = suspended.parent;
                continue;
            };

            *self.counts.entry(tag.code).or_default() += 1;
            if let Some(origin) = &mut self.origin {
                origin
                    .tags
                    .entry(tag.code)
                    .or_default()
                    .push(tag.site().header);
            }
            current.timeline.tag_count += 1;
            if tag.code == 39 {
                match self.read_sprite(&tag) {
                    Ok((sprite_index, child)) => parents.push(SuspendedTimeline {
                        sprite_index,
                        parent: std::mem::replace(&mut current, child),
                    }),
                    Err(error) => {
                        self.fail(tag.code, tag.offset, error, FailureOrigin::Tag(tag.site()));
                    }
                }
            } else {
                self.update_timeline(&tag, &mut current);
            }
        }
    }

    fn next_tag(&mut self, current: &mut TimelineScan<'a>) -> Option<Tag<'a>> {
        match current.stream.next_tag()? {
            Ok(tag) => Some(tag),
            Err(failed) => {
                let (code, offset) = match &failed.error {
                    GfxError::TagLength { code, offset, .. } => (*code, *offset),
                    _ => (0, current.end),
                };
                if let Some(origin) = &mut current.origin {
                    origin.complete = false;
                }
                self.fail(code, offset, failed.error, failed.origin);
                None
            }
        }
    }

    fn update_timeline(&mut self, tag: &Tag<'a>, current: &mut TimelineScan<'a>) {
        let result = self
            .read_tag(tag, &mut current.timeline)
            .and_then(|accepted| {
                if let (Some(origin), Some(timeline)) = (&mut self.origin, &mut current.origin) {
                    origin.accepted(tag.site(), accepted, &self.assets, timeline);
                }
                let first_frame = current
                    .origin
                    .as_mut()
                    .filter(|_| current.timeline.frames == 0);
                read_display_tag(tag, &mut current.display_list, first_frame)
            });
        if let Err(error) = result {
            self.fail(tag.code, tag.offset, error, FailureOrigin::Tag(tag.site()));
        }
        if tag.code == 1 {
            self.end_frame(&mut current.timeline, &current.display_list, tag);
        }
    }

    fn end_frame(&mut self, timeline: &mut Timeline, display_list: &DisplayList, tag: &Tag<'_>) {
        if timeline.frames == 1 {
            timeline.first_frame = display_list.snapshot();
        }
        if self.frames_capped {
            return;
        }
        let total = self.frame_placements.saturating_add(display_list.len() + 1);
        let bytes = self.frame_bytes.saturating_add(display_list.cloned_bytes());
        let error = if total > MAX_FRAME_PLACEMENTS {
            Some(GfxError::TooManyFramePlacements {
                limit: MAX_FRAME_PLACEMENTS,
            })
        } else if bytes > MAX_FRAME_BYTES {
            Some(GfxError::FrameListsTooLarge {
                limit: MAX_FRAME_BYTES,
            })
        } else {
            None
        };
        if let Some(error) = error {
            self.frames_capped = true;
            self.fail(tag.code, tag.offset, error, FailureOrigin::Tag(tag.site()));
        } else {
            self.frame_placements = total;
            self.frame_bytes = bytes;
            timeline.frame_lists.push(display_list.snapshot());
        }
    }

    fn fail(&mut self, code: u16, offset: usize, error: GfxError, site: FailureOrigin) {
        if let Some(origin) = &mut self.origin {
            origin.failure(code, site);
        }
        self.assets.failures.push(TagFailure {
            code,
            offset,
            error,
        });
    }

    fn read_sprite(&mut self, tag: &Tag<'a>) -> Result<(usize, TimelineScan<'a>), GfxError> {
        let mut reader = tag.reader();
        let id = reader.u16()?;
        let frame_count = reader.u16()?;
        let base = reader.offset();
        let index = self.assets.sprites.len();
        self.assets.sprites.push(Sprite {
            id,
            frame_count,
            timeline: Timeline::default(),
        });
        if let Some(origin) = &mut self.origin {
            origin.sprites.push(SpriteOrigins {
                definition: tag.site(),
                timeline: TimelineOrigins::default(),
            });
        }
        Ok((
            index,
            TimelineScan::new(reader.rest(), base, self.origin.is_some()),
        ))
    }
}
