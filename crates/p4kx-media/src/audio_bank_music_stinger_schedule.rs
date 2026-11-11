// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{BankHircField, BankMusicStinger, BankMusicStingerTail};

use crate::audio_bank_music_segment::MusicSites;
use crate::audio_bank_music_stinger_schedule_payload::{
    MusicStingerSchedulePayload, MusicStingerScheduleRecord, NAMESPACE, StingerRepeatTime,
};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_payload::BankField;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status};

struct Source<'fields> {
    range: Range<u64>,
    trigger_id: &'fields BankHircField<u32>,
    segment_id: Option<&'fields BankHircField<u32>>,
    sync_play_at: Option<&'fields BankHircField<u32>>,
    cue_filter_hash: Option<&'fields BankHircField<u32>>,
    repeat_bits: Option<&'fields BankHircField<u32>>,
    segment_look_ahead: Option<&'fields BankHircField<u32>>,
}

pub(crate) fn complete(
    input: &MediaOriginInput,
    sites: &MusicSites<'_>,
    ordinal: u64,
    original: &BankMusicStinger,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let source = Source {
        range: original.trigger_id.range.start..original.opaque_range.end,
        trigger_id: &original.trigger_id,
        segment_id: Some(&original.segment_id),
        sync_play_at: Some(&original.sync_play_at),
        cue_filter_hash: Some(&original.cue_filter_hash),
        repeat_bits: Some(&original.dont_repeat_time_bits),
        segment_look_ahead: Some(&original.segment_look_ahead),
    };
    emit(input, sites, ordinal, &source, writer)
}

pub(crate) fn tail(
    input: &MediaOriginInput,
    sites: &MusicSites<'_>,
    original: Option<&BankMusicStingerTail>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Some(original) = original else {
        return Ok(());
    };
    let source = Source {
        range: original.checked_range.clone(),
        trigger_id: &original.trigger_id,
        segment_id: original.segment_id.as_ref(),
        sync_play_at: original.sync_play_at.as_ref(),
        cue_filter_hash: original.cue_filter_hash.as_ref(),
        repeat_bits: original.dont_repeat_time_bits.as_ref(),
        segment_look_ahead: None,
    };
    emit(input, sites, u64::from(original.ordinal), &source, writer)
}

fn emit(
    input: &MediaOriginInput,
    sites: &MusicSites<'_>,
    ordinal: u64,
    source: &Source<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let malformed = source.segment_look_ahead.is_none();
    let mut prepared = sites.schedule_origins(input, ordinal, &source.range, malformed, writer)?;
    let record = read(sites, &mut prepared, ordinal, source)?;
    let (status, issue) = if malformed {
        (
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.truncated-music-stinger-schedule",
                "HIRC",
                Severity::Error,
                "the original stinger ends before all its scheduling words fit",
            )),
        )
    } else {
        (Status::Complete, None)
    };
    prepared.finish().write(
        writer,
        &sites.subresource(NAMESPACE, ordinal),
        status,
        &MusicStingerSchedulePayload::Schedule(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &MusicSites<'_>,
    prepared: &mut BankOrigins<'_, '_, '_>,
    ordinal: u64,
    original: &Source<'_>,
) -> Result<MusicStingerScheduleRecord, MediaError> {
    sites.schedule_word(
        prepared,
        ordinal,
        "trigger_id",
        "musicStingerScheduleTriggerId",
        &original.trigger_id.range,
    )?;
    let trigger_id = original.trigger_id.into();
    let [
        segment_id,
        sync_play_at,
        cue_filter_hash,
        segment_look_ahead,
    ] = observe_words(sites, prepared, ordinal, original)?;
    let dont_repeat_time = repeat(sites, prepared, ordinal, original.repeat_bits)?;
    let ordinal = DecimalU64::new(ordinal);
    match (
        segment_id,
        sync_play_at,
        cue_filter_hash,
        dont_repeat_time,
        segment_look_ahead,
    ) {
        (
            Some(segment_id),
            Some(sync_play_at),
            Some(cue_filter_hash),
            Some(dont_repeat_time),
            Some(segment_look_ahead),
        ) => Ok(MusicStingerScheduleRecord::Decoded {
            ordinal,
            trigger_id,
            segment_id,
            sync_play_at,
            cue_filter_hash,
            dont_repeat_time,
            segment_look_ahead,
        }),
        (segment_id, sync_play_at, cue_filter_hash, dont_repeat_time, None) => {
            Ok(MusicStingerScheduleRecord::Malformed {
                ordinal,
                trigger_id,
                segment_id,
                sync_play_at,
                cue_filter_hash,
                dont_repeat_time,
            })
        }
        _ => Err(MediaError::Contract(
            "complete stinger schedule lacks original words".into(),
        )),
    }
}

fn observe_words(
    sites: &MusicSites<'_>,
    prepared: &mut BankOrigins<'_, '_, '_>,
    ordinal: u64,
    original: &Source<'_>,
) -> Result<[Option<BankField<u32>>; 4], MediaError> {
    let mut fields = [const { None }; 4];
    for ((key, label, word), field) in [
        (
            "segment_id",
            "musicStingerScheduleSegmentId",
            original.segment_id,
        ),
        (
            "sync_play_at",
            "musicStingerSyncPlayAt",
            original.sync_play_at,
        ),
        (
            "cue_filter_hash",
            "musicStingerCueFilterHash",
            original.cue_filter_hash,
        ),
        (
            "segment_look_ahead",
            "musicStingerSegmentLookAhead",
            original.segment_look_ahead,
        ),
    ]
    .into_iter()
    .zip(&mut fields)
    {
        if let Some(word) = word {
            sites.schedule_word(prepared, ordinal, key, label, &word.range)?;
            *field = Some(word.into());
        }
    }
    Ok(fields)
}

fn repeat(
    sites: &MusicSites<'_>,
    prepared: &mut BankOrigins<'_, '_, '_>,
    ordinal: u64,
    original: Option<&BankHircField<u32>>,
) -> Result<Option<StingerRepeatTime>, MediaError> {
    let Some(word) = original else {
        return Ok(None);
    };
    sites.schedule_word(
        prepared,
        ordinal,
        "dont_repeat_time/bits",
        "musicStingerDontRepeatBits",
        &word.range,
    )?;
    sites.schedule_signed(prepared, ordinal, &word.range)?;
    Ok(Some(StingerRepeatTime {
        bits: word.into(),
        signed_value: i32::from_le_bytes(word.value.to_le_bytes()),
    }))
}
