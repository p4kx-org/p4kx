// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{BankContainerFields, BankHircField, BankRandomSequencePlaylist};

use crate::source_evidence_error::SourceEvidenceError;

use super::super::origins::ObjectOrigins;
use super::dto::{RandomSequenceFields, RandomSequencePlaylist, RandomSequenceTransition};

pub(super) fn collect(
    container: &BankContainerFields,
    origins: &ObjectOrigins,
) -> Result<RandomSequenceFields, SourceEvidenceError> {
    let Some(fields) = &container.random_sequence else {
        return Ok(RandomSequenceFields::default());
    };
    let mode = origins.optional(container.mode.as_ref(), "randomSequenceMode")?;
    let child_count = origins.optional(container.child_count.as_ref(), "containerChildCount")?;
    let child_range = container
        .children
        .as_ref()
        .map(|range| origins.range(range, "containerChildrenExtent"))
        .transpose()?;
    let playlist = fields
        .playlist
        .as_ref()
        .map(|extent| playlist(extent, origins))
        .transpose()?;
    Ok(RandomSequenceFields {
        loop_count: origins.optional(fields.loop_count.as_ref(), "randomSequenceLoopCount")?,
        loop_minimum: origins
            .optional(fields.loop_minimum.as_ref(), "randomSequenceLoopMinimum")?,
        loop_maximum: origins
            .optional(fields.loop_maximum.as_ref(), "randomSequenceLoopMaximum")?,
        transition_time: transition(
            fields.transition_time.as_ref(),
            origins,
            "randomSequenceTransitionTime",
        )?,
        transition_minimum: transition(
            fields.transition_minimum.as_ref(),
            origins,
            "randomSequenceTransitionMinimum",
        )?,
        transition_maximum: transition(
            fields.transition_maximum.as_ref(),
            origins,
            "randomSequenceTransitionMaximum",
        )?,
        avoid_repeat_count: origins.optional(
            fields.avoid_repeat_count.as_ref(),
            "randomSequenceAvoidRepeatCount",
        )?,
        transition_mode: origins.optional(
            fields.transition_mode.as_ref(),
            "randomSequenceTransitionMode",
        )?,
        random_mode: origins.optional(fields.random_mode.as_ref(), "randomSequenceRandomMode")?,
        mode,
        flags: origins.optional(fields.flags.as_ref(), "randomSequenceFlags")?,
        child_count,
        child_range,
        playlist,
    })
}

fn transition(
    field: Option<&BankHircField<u32>>,
    origins: &ObjectOrigins,
    label: &'static str,
) -> Result<Option<RandomSequenceTransition>, SourceEvidenceError> {
    let Some(field) = field else {
        return Ok(None);
    };
    let value = f32::from_bits(field.value);
    let finite_value = value
        .is_finite()
        .then(|| origins.derived(value, &field.range, "randomSequenceFiniteTransition"))
        .transpose()?;
    Ok(Some(RandomSequenceTransition {
        bits: origins.field(field, label)?,
        finite_value,
    }))
}

fn playlist(
    playlist: &BankRandomSequencePlaylist,
    origins: &ObjectOrigins,
) -> Result<RandomSequencePlaylist, SourceEvidenceError> {
    let count = &playlist.declared_count;
    let extent = count.range.start..playlist.checked_range.end;
    Ok(RandomSequencePlaylist {
        declared_count: origins.field(count, "randomSequencePlaylistCount")?,
        checked_range: origins.range(&playlist.checked_range, "randomSequencePlaylistExtent")?,
        complete: origins.derived(
            playlist.is_complete(),
            &extent,
            "randomSequencePlaylistCoverage",
        )?,
    })
}
