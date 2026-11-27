// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    BankHircField, BankMusicFadeFields, BankMusicTrackFields, BankMusicTrackSwitchFields,
    BankMusicTrackTransitionFields, BankTrackAutomationExtent,
};

use crate::source_evidence_error::SourceEvidenceError;

use super::super::origins::ObjectOrigins;
use super::dto::{
    MusicTrackSignedWord, MusicTrackSourceFade, MusicTrackSourceFields, MusicTrackSourceSwitch,
    MusicTrackSourceTable, MusicTrackSourceTransition,
};

pub(super) fn read(
    fields: &BankMusicTrackFields,
    origins: &ObjectOrigins,
) -> Result<MusicTrackSourceFields, SourceEvidenceError> {
    let node_range = fields
        .node_range
        .as_ref()
        .map(|range| origins.range(range, "musicTrackNodeExtent"))
        .transpose()?;
    let automation = fields
        .automation_count
        .as_ref()
        .zip(fields.automation_extent.as_ref())
        .map(|(count, extent)| {
            table(
                origins,
                count,
                extent,
                "musicTrackAutomationCount",
                "musicTrackAutomationExtent",
            )
        })
        .transpose()?;
    let switch = fields
        .switch
        .as_ref()
        .map(|fields| read_switch(fields, origins))
        .transpose()?;
    Ok(MusicTrackSourceFields {
        source_count: origins.optional(fields.source_count.as_ref(), "musicTrackSourceCount")?,
        clip_count: origins.optional(fields.clip_count.as_ref(), "musicTrackClipCount")?,
        flags: origins.optional(fields.flags.as_ref(), "musicTrackFlags")?,
        subtrack_count: origins
            .optional(fields.subtrack_count.as_ref(), "musicTrackSubtrackCount")?,
        parent_id: origins.optional(fields.parent.parent_id.as_ref(), "musicTrackParentId")?,
        override_bus_id: origins
            .optional(fields.parent.override_bus_id(), "musicTrackOverrideBusId")?,
        node_range,
        kind_code: origins.optional(fields.kind_code.as_ref(), "musicTrackKind")?,
        look_ahead: signed_word(
            origins,
            fields.look_ahead_bits.as_ref(),
            "musicTrackLookAheadBits",
        )?,
        automation,
        switch,
    })
}

pub(super) fn table(
    origins: &ObjectOrigins,
    count: &BankHircField<u32>,
    extent: &BankTrackAutomationExtent,
    count_label: &'static str,
    range_label: &'static str,
) -> Result<MusicTrackSourceTable, SourceEvidenceError> {
    let inspected = count.range.start..extent.checked_range.end;
    Ok(MusicTrackSourceTable {
        declared_count: origins.field(count, count_label)?,
        checked_range: origins.range(&extent.checked_range, range_label)?,
        complete: origins.derived(extent.is_complete(), &inspected, "musicTrackTableCoverage")?,
    })
}

fn read_switch(
    fields: &BankMusicTrackSwitchFields,
    origins: &ObjectOrigins,
) -> Result<MusicTrackSourceSwitch, SourceEvidenceError> {
    let transition = fields
        .transition
        .as_ref()
        .map(|fields| read_transition(fields, origins))
        .transpose()?;
    Ok(MusicTrackSourceSwitch {
        group_type: origins.optional(fields.group_type.as_ref(), "musicTrackSwitchGroupType")?,
        group_id: origins.optional(fields.group_id.as_ref(), "musicTrackSwitchGroupId")?,
        default_value_id: origins
            .optional(fields.default_switch.as_ref(), "musicTrackDefaultSwitchId")?,
        member_count: origins
            .optional(fields.member_count.as_ref(), "musicTrackSwitchMemberCount")?,
        transition,
        transition_range: fields
            .transition_range
            .as_ref()
            .map(|range| origins.range(range, "musicTrackSwitchTransitionExtent"))
            .transpose()?,
    })
}

fn read_transition(
    fields: &BankMusicTrackTransitionFields,
    origins: &ObjectOrigins,
) -> Result<MusicTrackSourceTransition, SourceEvidenceError> {
    Ok(MusicTrackSourceTransition {
        source_fade: fade(origins, &fields.source, FadeFields::Source)?,
        sync_type: origins.optional(fields.sync_type.as_ref(), "musicTrackTransitionSyncType")?,
        cue_filter_hash: origins.optional(
            fields.cue_filter_hash.as_ref(),
            "musicTrackTransitionCueFilterHash",
        )?,
        destination_fade: fade(origins, &fields.destination, FadeFields::Destination)?,
    })
}

#[derive(Clone, Copy)]
enum FadeFields {
    Source,
    Destination,
}

fn fade(
    origins: &ObjectOrigins,
    fields: &BankMusicFadeFields,
    site: FadeFields,
) -> Result<MusicTrackSourceFade, SourceEvidenceError> {
    let (time, curve, offset) = match site {
        FadeFields::Source => (
            "musicTrackSourceFadeTimeBits",
            "musicTrackSourceFadeCurve",
            "musicTrackSourceFadeOffsetBits",
        ),
        FadeFields::Destination => (
            "musicTrackDestinationFadeTimeBits",
            "musicTrackDestinationFadeCurve",
            "musicTrackDestinationFadeOffsetBits",
        ),
    };
    Ok(MusicTrackSourceFade {
        time: signed_word(origins, fields.time_bits.as_ref(), time)?,
        curve: origins.optional(fields.curve.as_ref(), curve)?,
        offset: signed_word(origins, fields.offset_bits.as_ref(), offset)?,
    })
}

fn signed_word(
    origins: &ObjectOrigins,
    original: Option<&BankHircField<u32>>,
    label: &'static str,
) -> Result<Option<MusicTrackSignedWord>, SourceEvidenceError> {
    original
        .map(|word| {
            let value = i32::from_le_bytes(word.value.to_le_bytes());
            Ok(MusicTrackSignedWord {
                bits: origins.field(word, label)?,
                signed_value: origins.derived(value, &word.range, label)?,
            })
        })
        .transpose()
}
