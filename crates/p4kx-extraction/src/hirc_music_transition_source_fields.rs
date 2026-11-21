// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    BankHircField, BankMusicFadeFields, BankMusicTransitionDestinationFields,
    BankMusicTransitionObjectFields, BankMusicTransitionSourceFields,
};

use crate::source_evidence_error::SourceEvidenceError;

use super::super::dto::{
    Fade, SignedWord, TransitionDestination, TransitionObject, TransitionSource,
};
use super::ObjectOrigins;

pub(super) fn source(
    origins: &ObjectOrigins,
    fields: &BankMusicTransitionSourceFields,
) -> Result<Option<TransitionSource>, SourceEvidenceError> {
    let Some(fade) = fade(
        origins,
        &fields.fade,
        [
            "musicTransitionSourceTimeBits",
            "musicTransitionSourceCurve",
            "musicTransitionSourceOffsetBits",
        ],
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TransitionSource {
        fade,
        sync_type: origins.optional(fields.sync_type.as_ref(), "musicTransitionSourceSyncType")?,
        cue_filter_hash: origins.optional(
            fields.cue_filter_hash.as_ref(),
            "musicTransitionSourceCueFilterHash",
        )?,
        play_post_exit: origins.optional(
            fields.play_post_exit.as_ref(),
            "musicTransitionSourcePlayPostExit",
        )?,
    }))
}

pub(super) fn destination(
    origins: &ObjectOrigins,
    fields: &BankMusicTransitionDestinationFields,
) -> Result<Option<TransitionDestination>, SourceEvidenceError> {
    let Some(fade) = fade(
        origins,
        &fields.fade,
        [
            "musicTransitionDestinationTimeBits",
            "musicTransitionDestinationCurve",
            "musicTransitionDestinationOffsetBits",
        ],
    )?
    else {
        return Ok(None);
    };
    Ok(Some(TransitionDestination {
        fade,
        cue_filter_hash: origins.optional(
            fields.cue_filter_hash.as_ref(),
            "musicTransitionDestinationCueFilterHash",
        )?,
        jump_to_id: origins.optional(fields.jump_to_id.as_ref(), "musicTransitionJumpToId")?,
        jump_to_type: origins
            .optional(fields.jump_to_type.as_ref(), "musicTransitionJumpToType")?,
        entry_type: origins.optional(fields.entry_type.as_ref(), "musicTransitionEntryType")?,
        play_pre_entry: origins.optional(
            fields.play_pre_entry.as_ref(),
            "musicTransitionDestinationPlayPreEntry",
        )?,
        match_source_cue_name: origins.optional(
            fields.match_source_cue_name.as_ref(),
            "musicTransitionMatchSourceCueName",
        )?,
    }))
}

pub(super) fn transition_object(
    origins: &ObjectOrigins,
    fields: &BankMusicTransitionObjectFields,
) -> Result<TransitionObject, SourceEvidenceError> {
    Ok(TransitionObject {
        segment_id: origins
            .optional(fields.segment_id.as_ref(), "musicTransitionObjectSegmentId")?,
        fade_in: fade(
            origins,
            &fields.fade_in,
            [
                "musicTransitionObjectFadeInTimeBits",
                "musicTransitionObjectFadeInCurve",
                "musicTransitionObjectFadeInOffsetBits",
            ],
        )?,
        fade_out: fade(
            origins,
            &fields.fade_out,
            [
                "musicTransitionObjectFadeOutTimeBits",
                "musicTransitionObjectFadeOutCurve",
                "musicTransitionObjectFadeOutOffsetBits",
            ],
        )?,
        play_pre_entry: origins.optional(
            fields.play_pre_entry.as_ref(),
            "musicTransitionObjectPlayPreEntry",
        )?,
        play_post_exit: origins.optional(
            fields.play_post_exit.as_ref(),
            "musicTransitionObjectPlayPostExit",
        )?,
    })
}

fn fade(
    origins: &ObjectOrigins,
    original: &BankMusicFadeFields,
    [time, curve, offset]: [&'static str; 3],
) -> Result<Option<Fade>, SourceEvidenceError> {
    let Some(word) = &original.time_bits else {
        return Ok(None);
    };
    Ok(Some(Fade {
        time: signed_word(origins, word, time)?,
        curve: origins.optional(original.curve.as_ref(), curve)?,
        offset: original
            .offset_bits
            .as_ref()
            .map(|word| signed_word(origins, word, offset))
            .transpose()?,
    }))
}

fn signed_word(
    origins: &ObjectOrigins,
    word: &BankHircField<u32>,
    label: &'static str,
) -> Result<SignedWord, SourceEvidenceError> {
    Ok(SignedWord {
        bits: origins.field(word, label)?,
        signed_value: origins.derived(word.value.cast_signed(), &word.range, label)?,
    })
}
