// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};

const HAS_POSITIONING: u8 = 0x01;
const HAS_3D: u8 = 0x02;
const PANNER_SHIFT: u8 = 2;
const POSITION_TYPE_SHIFT: u8 = 5;
const MODE_MASK: u8 = 0b11;

pub(crate) const PATH_WORD_LEN: usize = size_of::<u32>();
pub(crate) const VERTEX_LEN: usize = 4 * PATH_WORD_LEN;

/// A physical table in the version-154 positioning automation region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankPositioningTableKind {
    /// Three binary32 coordinate words and one signed duration.
    Vertex,
    /// Original vertex offset and count.
    Playlist,
    /// Three binary32 automation range words.
    AutomationRange,
}

impl BankPositioningTableKind {
    pub(crate) const fn row_len(self) -> usize {
        match self {
            Self::Vertex => VERTEX_LEN,
            Self::Playlist => 2 * PATH_WORD_LEN,
            Self::AutomationRange => 3 * PATH_WORD_LEN,
        }
    }
}

/// Version-154 mode codes and positioning controls from the original flag byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankPositioningModes {
    /// Whether authored positioning overrides the parent.
    pub override_parent: bool,
    /// Whether this node body carries the separate 3D flags.
    pub has_spatial_fields: bool,
    /// Original two-bit panner code.
    pub panner_type_code: u8,
    /// Original two-bit 3D position code.
    pub position_type_code: u8,
}

impl BankPositioningModes {
    const fn decode(flags: u8) -> Self {
        let override_parent = flags & HAS_POSITIONING != 0;
        Self {
            override_parent,
            has_spatial_fields: override_parent && flags & HAS_3D != 0,
            panner_type_code: (flags >> PANNER_SHIFT) & MODE_MASK,
            position_type_code: (flags >> POSITION_TYPE_SHIFT) & MODE_MASK,
        }
    }
}

/// Original fields and checked path table prefixes of a version-154 node.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankNodePositioningFields {
    /// Bank-relative start when the region was reached.
    pub start: Option<u64>,
    /// Original positioning flags, including unknown bits.
    pub flags: Option<BankHircField<u8>>,
    /// Original 3D flags when both positioning flags select that byte.
    pub spatial_flags: Option<BankHircField<u8>>,
    /// Original automation mode byte when the path region was reached.
    pub path_mode: Option<BankHircField<u8>>,
    /// Original signed transition time when readable.
    pub transition_time: Option<BankHircField<i32>>,
    /// Original path vertex count, including zero.
    pub vertex_count: Option<BankHircField<u32>>,
    /// Original count shared by playlist and automation ranges.
    pub playlist_count: Option<BankHircField<u32>>,
    vertices: Option<Range<u64>>,
    playlist: Option<Range<u64>>,
    automation: Option<Range<u64>>,
    end: Option<u64>,
}

impl BankNodePositioningFields {
    /// Returns the checked table bytes, including any incomplete final word.
    #[must_use]
    pub const fn table(&self, kind: BankPositioningTableKind) -> Option<&Range<u64>> {
        match kind {
            BankPositioningTableKind::Vertex => self.vertices.as_ref(),
            BankPositioningTableKind::Playlist => self.playlist.as_ref(),
            BankPositioningTableKind::AutomationRange => self.automation.as_ref(),
        }
    }

    /// Returns the positioning mode codes when their original flag byte is readable.
    #[must_use]
    pub const fn modes(&self) -> Option<BankPositioningModes> {
        match &self.flags {
            Some(flags) => Some(BankPositioningModes::decode(flags.value)),
            None => None,
        }
    }

    /// Returns the complete positioning extent when all selected fields fit.
    #[must_use]
    pub fn complete_range(&self) -> Option<Range<u64>> {
        Some(self.start?..self.end?)
    }
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankNodePositioningFields,
) -> Result<(), F::Error> {
    observed.start = Some(input.offset());
    let flags = input.read_u8()?;
    let modes = BankPositioningModes::decode(flags.value);
    observed.flags = Some(flags);
    if modes.has_spatial_fields {
        observed.spatial_flags = Some(input.read_u8()?);
        if modes.position_type_code != 0 {
            scan_path(input, observed)?;
        }
    }
    observed.end = Some(input.offset());
    Ok(())
}

fn scan_path<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankNodePositioningFields,
) -> Result<(), F::Error> {
    observed.path_mode = Some(input.read_u8()?);
    observed.transition_time = Some(input.scalar(i32::from_le_bytes)?);
    let count = input.read_u32()?;
    let vertices = count.value;
    observed.vertex_count = Some(count);
    scan_table(
        input,
        BankPositioningTableKind::Vertex,
        vertices,
        &mut observed.vertices,
    )?;
    let count = input.read_u32()?;
    let playlist = count.value;
    observed.playlist_count = Some(count);
    scan_table(
        input,
        BankPositioningTableKind::Playlist,
        playlist,
        &mut observed.playlist,
    )?;
    scan_table(
        input,
        BankPositioningTableKind::AutomationRange,
        playlist,
        &mut observed.automation,
    )
}

fn scan_table<F: ObjectFields>(
    input: &mut F,
    kind: BankPositioningTableKind,
    count: u32,
    observed: &mut Option<Range<u64>>,
) -> Result<(), F::Error> {
    let required = u64::from(count) * kind.row_len() as u64;
    let start = input.offset();
    *observed = Some(start..start + input.remaining_bytes().min(required));
    input.skip_bytes(required)
}
