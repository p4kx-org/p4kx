// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::BankError;
use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::bank_reader::SectionReader;
use crate::hirc_music::{MusicTrack, MusicTrackKind, MusicTrackSwitch};
use crate::hirc_music_segment;
use crate::hirc_music_track_automation::{self, BankTrackAutomationExtent};
use crate::hirc_music_track_transition::{self, BankMusicTrackTransitionFields};
use crate::hirc_node::{self, BankNodeParentFields};

#[path = "hirc_music_track_rows.rs"]
mod rows;

pub use rows::{BankMusicTrackClip, BankMusicTrackSource};
pub(crate) use rows::{clip, source};

const SWITCH_TRACK: u8 = 3;

/// Original header fields read while locating `MusicTrack` rows.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTrackFields {
    /// Original source count.
    pub source_count: Option<BankHircField<u32>>,
    /// Original clip count.
    pub clip_count: Option<BankHircField<u32>>,
    /// Start of the original control continuation after sources.
    pub controls_start: Option<u64>,
    /// Original track flags, including unknown bits.
    pub flags: Option<BankHircField<u8>>,
    /// Original subtrack count when the clip list is nonempty.
    pub subtrack_count: Option<BankHircField<u32>>,
    /// Original signed look-ahead bits, including explicit zero.
    pub look_ahead_bits: Option<BankHircField<u32>>,
    /// Original automation count.
    pub automation_count: Option<BankHircField<u32>>,
    /// Checked opaque automation extent.
    pub automation_range: Option<Range<u64>>,
    /// Checked original automation table including a partial row prefix.
    pub automation_extent: Option<BankTrackAutomationExtent>,
    /// Original parent and preceding node selectors.
    pub parent: BankNodeParentFields,
    /// Original checked groups following the parent declaration.
    pub parameters: crate::BankNodeParameters,
    /// Traversed node prefix.
    pub node_range: Option<Range<u64>>,
    /// Original track kind byte.
    pub kind_code: Option<BankHircField<u8>>,
    /// Original switch fields when the switch layout is selected.
    pub switch: Option<BankMusicTrackSwitchFields>,
}

impl BankMusicTrackFields {
    /// Returns the known track kind or its original unknown code.
    #[must_use]
    pub fn kind(&self) -> Option<MusicTrackKind> {
        self.kind_code.as_ref().map(|field| kind(field.value))
    }
}

/// Original header of the selected `MusicTrack` switch block.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTrackSwitchFields {
    /// Original switch or state group type.
    pub group_type: Option<BankHircField<u8>>,
    /// Original group ID.
    pub group_id: Option<BankHircField<u32>>,
    /// Original default switch ID.
    pub default_switch: Option<BankHircField<u32>>,
    /// Original count of switch member IDs.
    pub member_count: Option<BankHircField<u32>>,
    /// Checked opaque transition extent.
    pub transition_range: Option<Range<u64>>,
    /// Original transition words and their checked prefix when reached.
    pub transition: Option<BankMusicTrackTransitionFields>,
}

/// One ordered original row of a physical `MusicTrack`.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankMusicTrackRow {
    /// One original media source.
    Source {
        /// Position in the source list.
        ordinal: u32,
        /// Original source words.
        value: BankMusicTrackSource,
    },
    /// One original clip.
    Clip {
        /// Position in the clip list.
        ordinal: u32,
        /// Original clip words.
        value: BankMusicTrackClip,
    },
    /// One original switch member.
    Switch {
        /// Position in the member list.
        ordinal: u32,
        /// Original switch or state value ID.
        id: BankHircField<u32>,
    },
}

pub(crate) struct TrackHeader {
    parent: u32,
    kind: MusicTrackKind,
    switch: Option<SwitchHeader>,
}

struct SwitchHeader {
    group_type: u8,
    group_id: u32,
    default_switch: u32,
}

pub(crate) fn track(version: u32, input: &mut SectionReader<'_>) -> Result<MusicTrack, BankError> {
    let mut sources = Vec::new();
    let mut clips = Vec::new();
    let mut members = Vec::new();
    let header = read(
        version,
        input,
        &mut BankMusicTrackFields::default(),
        |row| match row {
            BankMusicTrackRow::Source { value, .. } => sources.push(value.value),
            BankMusicTrackRow::Clip { value, .. } => clips.push(value.value()),
            BankMusicTrackRow::Switch { id, .. } => members.push(id.value),
        },
    )?;
    Ok(MusicTrack {
        parent_id: header.parent,
        sources,
        clips,
        kind: header.kind,
        switch: header.switch.map(|header| MusicTrackSwitch {
            group_type: header.group_type,
            group_id: header.group_id,
            default_switch: header.default_switch,
            switches: members,
        }),
    })
}

pub(crate) fn read<F: ObjectFields>(
    version: u32,
    input: &mut F,
    fields: &mut BankMusicTrackFields,
    mut emit: impl FnMut(BankMusicTrackRow),
) -> Result<TrackHeader, F::Error> {
    let sources = count(input, &mut fields.source_count)?;
    for ordinal in 0..sources {
        emit(BankMusicTrackRow::Source {
            ordinal,
            value: source(version, input)?,
        });
    }
    fields.controls_start = Some(input.offset());
    fields.flags = Some(input.read_u8()?);
    read_clips(version, input, fields, &mut emit)?;
    automations(input, fields)?;
    let start = input.offset();
    let parent = hirc_node::read_base_parameters(input, &mut fields.parent, &mut fields.parameters);
    fields.node_range = Some(start..input.offset());
    let parent = parent?;
    let code = input.read_u8()?;
    let kind = kind(code.value);
    fields.kind_code = Some(code);
    let switch = if kind == MusicTrackKind::Switch {
        let observed = fields.switch.insert(BankMusicTrackSwitchFields::default());
        Some(read_switch(input, observed, &mut emit)?)
    } else {
        None
    };
    fields.look_ahead_bits = Some(input.read_u32()?);
    hirc_music_segment::finish(input)?;
    Ok(TrackHeader {
        parent,
        kind,
        switch,
    })
}

fn read_clips<F: ObjectFields>(
    version: u32,
    input: &mut F,
    fields: &mut BankMusicTrackFields,
    emit: &mut impl FnMut(BankMusicTrackRow),
) -> Result<(), F::Error> {
    let clips = count(input, &mut fields.clip_count)?;
    for ordinal in 0..clips {
        emit(BankMusicTrackRow::Clip {
            ordinal,
            value: clip(version, input)?,
        });
    }
    if clips > 0 {
        fields.subtrack_count = Some(input.read_u32()?);
    }
    Ok(())
}

fn count<F: ObjectFields>(
    input: &mut F,
    original: &mut Option<BankHircField<u32>>,
) -> Result<u32, F::Error> {
    let word = input.read_u32()?;
    let value = word.value;
    *original = Some(word);
    Ok(value)
}

fn automations<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankMusicTrackFields,
) -> Result<(), F::Error> {
    let count = count(input, &mut fields.automation_count)?;
    let (extent, result) = hirc_music_track_automation::scan(input, count);
    if extent.is_complete() {
        fields.automation_range = Some(extent.checked_range.clone());
    }
    fields.automation_extent = Some(extent);
    result
}

fn read_switch<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankMusicTrackSwitchFields,
    emit: &mut impl FnMut(BankMusicTrackRow),
) -> Result<SwitchHeader, F::Error> {
    let original_type = input.read_u8()?;
    let group_type = original_type.value;
    fields.group_type = Some(original_type);
    let group_id = count(input, &mut fields.group_id)?;
    let default_switch = count(input, &mut fields.default_switch)?;
    let members = count(input, &mut fields.member_count)?;
    hirc_node::checked_child_bytes(members, input.remaining_bytes())?;
    for ordinal in 0..members {
        emit(BankMusicTrackRow::Switch {
            ordinal,
            id: input.read_u32()?,
        });
    }
    let start = input.offset();
    let transition = fields
        .transition
        .insert(BankMusicTrackTransitionFields::default());
    hirc_music_track_transition::read(input, transition)?;
    fields.transition_range = Some(start..input.offset());
    Ok(SwitchHeader {
        group_type,
        group_id,
        default_switch,
    })
}

const fn kind(code: u8) -> MusicTrackKind {
    match code {
        0 => MusicTrackKind::Normal,
        1 => MusicTrackKind::Random,
        2 => MusicTrackKind::Sequence,
        SWITCH_TRACK => MusicTrackKind::Switch,
        other => MusicTrackKind::Other(other),
    }
}
