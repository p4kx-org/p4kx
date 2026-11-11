// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::{Assets, MovieSpan, TagItemSite, TagSite};

/// Consecutive observed ranges of the same length at a fixed stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SpanRun {
    /// The first observed range.
    pub first: MovieSpan,
    /// Distance between range starts, zero for one range.
    pub stride: usize,
    /// Number of observed ranges.
    pub count: usize,
}

/// Ordered range receipts, compacted where their offsets have a regular stride.
#[derive(Debug, Default)]
pub struct SpanRuns(Vec<SpanRun>);

impl SpanRuns {
    pub(crate) fn push(&mut self, next: MovieSpan) {
        if let Some(last) = self.0.last_mut()
            && last.first.length == next.length
            && let Some(delta) = next.offset.checked_sub(last.first.offset)
            && delta != 0
        {
            if last.count == 1 {
                last.stride = delta;
                last.count = 2;
                return;
            }
            if last.stride.checked_mul(last.count) == Some(delta) {
                last.count += 1;
                return;
            }
        }
        self.0.push(SpanRun {
            first: next,
            stride: 0,
            count: 1,
        });
    }

    /// Returns the actual ordered byte-range runs.
    #[must_use]
    pub fn runs(&self) -> &[SpanRun] {
        &self.0
    }
}

/// Source of one frame label and the accepted frame prefix that determines its index.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct LabelOrigin {
    /// The label declaration.
    pub tag: TagSite,
    /// Logical scan preceding the label in its own timeline.
    pub frame_prefix: MovieSpan,
}

/// Original inputs accepted in one timeline.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct TimelineOrigins {
    /// Logical range surveyed within this timeline.
    pub scan: MovieSpan,
    /// Whether the scan reached the stream boundary or an End tag.
    pub complete: bool,
    /// The accepted `ShowFrame` headers.
    pub frames: SpanRuns,
    /// The accepted label declarations, in order.
    pub labels: Vec<LabelOrigin>,
    /// Each accepted placement occurrence grouped by the ID emitted in the deduplicated list.
    pub placements: BTreeMap<u16, Vec<TagSite>>,
}

/// The physical defining tag and the tags that actually overrode its displayed font fields.
#[derive(Debug)]
#[non_exhaustive]
pub struct FontOrigins {
    /// This physical font occurrence.
    pub definition: TagSite,
    /// Last `FontInfo` applied to its name and style, if any.
    pub info: Option<TagSite>,
    /// Last `FontName` applied to its full name, if any.
    pub full_name: Option<TagSite>,
}

/// Physical occurrence of a sprite and the survey of its own nested timeline.
#[derive(Debug)]
#[non_exhaustive]
pub struct SpriteOrigins {
    /// The admitted sprite header.
    pub definition: TagSite,
    /// The accepted contents of this sprite timeline.
    pub timeline: TimelineOrigins,
}

/// The source material available to a retained diagnostic.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum FailureOrigin {
    /// An admitted tag whose body or interpreted branch reported a failure.
    Tag(TagSite),
    /// An original retained prefix and a measured remainder after tag admission failed.
    Stream {
        /// The bytes actually consumed as tag header fields.
        header: MovieSpan,
        /// The extent available at the failed read.
        available: MovieSpan,
    },
}

/// Source receipts aligned to the rows admitted by the movie's asset walk.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct AssetOrigins {
    /// Accepted tag headers grouped by reported tag code.
    pub tags: BTreeMap<u16, SpanRuns>,
    /// Accepted headers counted in the unread-tag report.
    pub unread: BTreeMap<u16, SpanRuns>,
    /// Unknown tags in output order.
    pub unknown: Vec<TagSite>,
    /// The main timeline.
    pub timeline: TimelineOrigins,
    /// Physical shape occurrences in output order.
    pub shapes: Vec<TagSite>,
    /// Physical morph shape occurrences in output order.
    pub morph_shapes: Vec<TagSite>,
    /// Physical bitmap occurrences in output order.
    pub bitmaps: Vec<TagSite>,
    /// Physical font occurrences and applied overrides in output order.
    pub fonts: Vec<FontOrigins>,
    /// Physical sprite occurrences in output order.
    pub sprites: Vec<SpriteOrigins>,
    /// The first accepted export record for each authored ID.
    pub exports: BTreeMap<u16, TagItemSite>,
    /// Physical import occurrences in output order.
    pub imports: Vec<TagSite>,
    /// Physical symbol-class items in output order.
    pub symbol_classes: Vec<TagItemSite>,
    /// Physical scaling grid occurrences in output order.
    pub scaling_grids: Vec<TagSite>,
    /// Last admitted exporter-info tag.
    pub exporter: Option<TagSite>,
    /// Retained diagnostics in output order.
    pub failures: Vec<FailureOrigin>,
    /// Whether any timeline stopped on a malformed tag header.
    pub truncated: bool,
    failed_codes: BTreeSet<u16>,
}

impl AssetOrigins {
    /// Returns whether an occurrence of this tag code failed during interpretation.
    #[must_use]
    pub fn failed(&self, code: u16) -> bool {
        self.failed_codes.contains(&code)
    }

    pub(crate) fn failure(&mut self, code: u16, site: FailureOrigin) {
        self.failed_codes.insert(code);
        self.truncated |= matches!(site, FailureOrigin::Stream { .. });
        self.failures.push(site);
    }

    pub(crate) fn accepted(
        &mut self,
        tag: TagSite,
        accepted: AcceptedTag,
        assets: &Assets<'_>,
        timeline: &mut TimelineOrigins,
    ) {
        match accepted {
            AcceptedTag::Other | AcceptedTag::Placement(None) => {}
            AcceptedTag::Frame => timeline.frames.push(tag.header),
            AcceptedTag::Placement(Some(id)) => {
                timeline.placements.entry(id).or_default().push(tag);
            }
            AcceptedTag::Label => timeline.labels.push(LabelOrigin {
                tag,
                frame_prefix: MovieSpan::new(timeline.scan.offset, tag.header.offset),
            }),
            AcceptedTag::Import => self.imports.push(tag),
            AcceptedTag::SymbolClasses(count) => {
                self.symbol_classes
                    .extend((0..count).map(|ordinal| TagItemSite::new(tag, ordinal)));
            }
            AcceptedTag::ScalingGrid => self.scaling_grids.push(tag),
            AcceptedTag::Shape => self.shapes.push(tag),
            AcceptedTag::Morph => self.morph_shapes.push(tag),
            AcceptedTag::Bitmap => self.bitmaps.push(tag),
            AcceptedTag::Font => self.fonts.push(FontOrigins {
                definition: tag,
                info: None,
                full_name: None,
            }),
            AcceptedTag::Exports(start) => {
                for (ordinal, record) in assets.exports.iter().skip(start).enumerate() {
                    self.exports
                        .entry(record.id)
                        .or_insert_with(|| TagItemSite::new(tag, ordinal));
                }
            }
            AcceptedTag::Exporter => self.exporter = Some(tag),
            AcceptedTag::Unread => self.unread.entry(tag.code).or_default().push(tag.header),
            AcceptedTag::Unknown => self.unknown.push(tag),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum AcceptedTag {
    Other,
    Frame,
    Placement(Option<u16>),
    Label,
    Import,
    SymbolClasses(usize),
    ScalingGrid,
    Shape,
    Morph,
    Bitmap,
    Font,
    Exports(usize),
    Exporter,
    Unread,
    Unknown,
}
