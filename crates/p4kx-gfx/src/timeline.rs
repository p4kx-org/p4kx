// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::asset_origins::TimelineOrigins;
use crate::place::{PlaceCommand, Placement};
use crate::tag::Tag;
use crate::{GfxError, TagSite};

const PLACE_HAS_CHARACTER: u8 = 0x02;
const PLACE3_HAS_CLASS_NAME: u8 = 0x08;
const PLACE3_HAS_IMAGE: u8 = 0x10;

/// A sprite: a movie clip with its own timeline, defined under an ID.
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    /// The ID of the sprite.
    pub id: u16,
    /// The frames its header declares.
    pub frame_count: u16,
    /// What its timeline holds.
    pub timeline: Timeline,
}

/// The most placements and frames the display lists of all frames of a movie hold together; the
/// display lists of later frames are not kept.
pub const MAX_FRAME_PLACEMENTS: usize = 1 << 20;

/// The most bytes the display lists of all frames of a movie occupy together, names and filter
/// data included; the display lists of later frames are not kept.
pub const MAX_FRAME_BYTES: usize = 64 << 20;

/// A frame label: a name for the frame it precedes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameLabel {
    /// The index of the frame it names, counted from 0.
    pub frame: u32,
    /// The name.
    pub name: String,
}

/// What a timeline holds: its frames, labels and the definitions it places.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Timeline {
    /// The `ShowFrame` tags.
    pub frames: u32,
    /// The frame labels in order, each with the frame it names.
    pub labels: Vec<FrameLabel>,
    /// The IDs of the definitions placed on it, each once, in ascending order.
    pub placed: Vec<u16>,
    /// The display list of the first frame in ascending depth order.
    pub first_frame: Vec<Placement>,
    /// The display list at the end of each frame, indexed by frame, each in ascending depth order.
    ///
    /// Shorter than [`Timeline::frames`] when the movie exceeds [`MAX_FRAME_PLACEMENTS`].
    pub frame_lists: Vec<Vec<Placement>>,
    /// The tags, End included.
    pub tag_count: u32,
}

impl Timeline {
    /// Returns the index of the first frame labelled `name`, ignoring ASCII case.
    #[must_use]
    pub fn frame_of(&self, name: &str) -> Option<u32> {
        self.labels
            .iter()
            .find(|label| label.name.eq_ignore_ascii_case(name))
            .map(|label| label.frame)
    }

    /// Returns the display list at the end of `frame`.
    #[must_use]
    pub fn display_list(&self, frame: u32) -> Option<&[Placement]> {
        self.frame_lists
            .get(usize::try_from(frame).ok()?)
            .map(Vec::as_slice)
    }
}

/// The display list a timeline builds up while its tags are read.
#[derive(Debug, Default)]
pub(crate) struct DisplayList(BTreeMap<u16, Placement>);

impl DisplayList {
    /// Returns the number of placements.
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the bytes a snapshot of the list occupies.
    pub(crate) fn cloned_bytes(&self) -> usize {
        self.0.values().fold(0usize, |sum, placement| {
            sum.saturating_add(placement.cloned_bytes())
        })
    }

    /// Returns the placements in ascending depth order.
    pub(crate) fn snapshot(&self) -> Vec<Placement> {
        self.0.values().cloned().collect()
    }

    /// Applies a `PlaceObject` tag.
    pub(crate) fn place(
        &mut self,
        command: PlaceCommand,
        site: TagSite,
        origin: Option<&mut TimelineOrigins>,
    ) {
        let depth = command.depth;
        let observe = |retained, command: &PlaceCommand| {
            if let Some(origin) = origin {
                origin.place(command, site, retained);
            }
        };
        if let Some(placement) = command.apply(self.0.remove(&depth), observe) {
            self.0.insert(depth, placement);
        }
    }

    /// Applies a `RemoveObject` or `RemoveObject2` tag.
    pub(crate) fn remove(
        &mut self,
        tag: &Tag<'_>,
        origin: Option<&mut TimelineOrigins>,
    ) -> Result<(), GfxError> {
        let mut reader = tag.reader();
        if tag.code == 5 {
            reader.u16()?;
        }
        let depth = reader.u16()?;
        self.0.remove(&depth);
        if let Some(origin) = origin {
            origin.first_frame.remove(&depth);
        }
        Ok(())
    }
}

impl Timeline {
    /// Sorts the placed IDs and removes repeats once the timeline is read; a timeline without a
    /// `ShowFrame` takes its first frame from the final display list.
    pub(crate) fn finish(&mut self, display_list: DisplayList) {
        self.placed.sort_unstable();
        self.placed.dedup();
        if self.frames == 0 {
            self.first_frame = display_list.0.into_values().collect();
        }
    }
}

/// Returns the ID of the definition a `PlaceObject`, `PlaceObject2` or `PlaceObject3` tag places, if
/// it places one rather than changing a placement.
pub(crate) fn placed_id(tag: &Tag<'_>) -> Result<Option<u16>, GfxError> {
    let mut reader = tag.reader();
    match tag.code {
        4 => reader.u16().map(Some),
        26 => {
            let flags = reader.u8()?;
            reader.u16()?;
            if flags & PLACE_HAS_CHARACTER == 0 {
                return Ok(None);
            }
            reader.u16().map(Some)
        }
        _ => {
            let flags = reader.u8()?;
            let flags3 = reader.u8()?;
            reader.u16()?;
            let has_character = flags & PLACE_HAS_CHARACTER != 0;
            if flags3 & PLACE3_HAS_CLASS_NAME != 0
                || (flags3 & PLACE3_HAS_IMAGE != 0 && has_character)
            {
                reader.c_string()?;
            }
            if !has_character {
                return Ok(None);
            }
            reader.u16().map(Some)
        }
    }
}

/// A name a movie exports a definition under, of an `ExportAssets` tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Export {
    /// The ID of the definition.
    pub id: u16,
    /// The name.
    pub name: String,
}

/// Reads the names of an `ExportAssets` tag.
pub(crate) fn read_exports(tag: &Tag<'_>) -> Result<Vec<Export>, GfxError> {
    let mut reader = tag.reader();
    let count = usize::from(reader.u16()?);
    let mut exports = Vec::with_capacity(count.min(reader.remaining()));
    for _ in 0..count {
        exports.push(Export {
            id: reader.u16()?,
            name: reader.c_string()?.into_owned(),
        });
    }
    Ok(exports)
}

/// What the GFx exporter states about a movie, of an `ExporterInfo` tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExporterInfo {
    /// The exporter version.
    pub version: u16,
    /// The export flags, from version 0x10A on.
    pub flags: u32,
    /// The format the exporter wrote the bitmaps in, as GFx numbers file formats.
    pub bitmap_format: u16,
    /// The prefix of the exported file names.
    pub prefix: String,
    /// The name of the SWF the movie was exported from.
    pub swf_name: String,
}

/// Reads `ExporterInfo`.
pub(crate) fn read_exporter_info(tag: &Tag<'_>) -> Result<ExporterInfo, GfxError> {
    let mut reader = tag.reader();
    let version = reader.u16()?;
    let flags = if version >= 0x10A { reader.u32()? } else { 0 };
    Ok(ExporterInfo {
        version,
        flags,
        bitmap_format: reader.u16()?,
        prefix: reader.short_string()?,
        swf_name: reader.short_string()?,
    })
}
