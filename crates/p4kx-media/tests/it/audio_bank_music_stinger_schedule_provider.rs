// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{HircLayout, Soundbank};
use p4kx_media::{MediaRecord, Status};
use p4kx_testkit::TestHircObject;
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{Evidence, bank, export, layout};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

const OWNER_ID: u32 = 0xA700_1110;
const TRIGGER_ID: u32 = 0x5100_0001;
const SEGMENT_ID: u32 = 0x5100_0002;
const NAMESPACE: &str = "audio-bank-music-stinger-schedule";
const SEGMENT: &str = "audio-bank-music-segment-stinger";
const SWITCH: &str = "audio-bank-music-switch-stinger";
const PLAYLIST: &str = "audio-bank-music-playlist-stinger";

#[derive(Clone, Copy)]
struct Schedule {
    sync: u32,
    cue: u32,
    repeat: i32,
    look_ahead: u32,
}

const NONZERO: Schedule = Schedule {
    sync: u32::MAX,
    cue: 0xB600_2119,
    repeat: i32::MIN,
    look_ahead: 7,
};
const ZERO: Schedule = Schedule {
    sync: 0,
    cue: 0,
    repeat: 0,
    look_ahead: 0,
};

#[test]
fn version_154_shared_schedule_reaches_each_music_owner_with_exact_origins() -> TestResult {
    for (kind, legacy, object, schedule, encrypted) in [
        (
            10,
            SEGMENT,
            TestHircObject::music_segment(OWNER_ID, &[], &[]),
            NONZERO,
            true,
        ),
        (
            12,
            SWITCH,
            TestHircObject::music_switch(OWNER_ID, &[SEGMENT_ID], 0xB900_1122, &[SEGMENT_ID]),
            ZERO,
            false,
        ),
        (
            13,
            PLAYLIST,
            TestHircObject::music_playlist(OWNER_ID, &[SEGMENT_ID]),
            NONZERO,
            true,
        ),
    ] {
        let bytes = source(154, kind, object, &[schedule], None)?;
        export(&bytes, encrypted, HircLayout::Decoded, |rows, evidence| {
            let record = row(rows, NAMESPACE, &[1, 0, 0])?;
            layout(record.1, "decoded", Status::Complete);
            check_schedule(record, evidence, schedule, true)?;
            let original = row(rows, legacy, &[1, 0, 0])?.1;
            assert_eq!(
                original.data.pointer("/payload/opaque_range/start"),
                record.1.data.pointer("/payload/sync_play_at/range/start"),
                "existing opaque range starts at the same physical word",
            );
            assert_eq!(
                original.data.pointer("/payload/opaque_range/end_exclusive"),
                record
                    .1
                    .data
                    .pointer("/payload/segment_look_ahead/range/end_exclusive"),
                "existing opaque range ends after the exact scheduling words",
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn truncated_final_stinger_preserves_earlier_duplicate_and_readable_schedule_prefix() -> TestResult
{
    let original = TestHircObject::music_segment_with_stingers(
        OWNER_ID,
        &[],
        &[(TRIGGER_ID, SEGMENT_ID), (TRIGGER_ID, SEGMENT_ID)],
    );
    let partial = Schedule {
        repeat: -7,
        ..NONZERO
    };
    let bytes = source(154, 10, original, &[ZERO, partial], Some(14))?;
    export(&bytes, true, HircLayout::Malformed, |rows, evidence| {
        let complete = row(rows, NAMESPACE, &[1, 0, 0])?;
        layout(complete.1, "decoded", Status::Complete);
        check_schedule(complete, evidence, ZERO, true)?;
        let tail = row(rows, NAMESPACE, &[1, 0, 1])?;
        layout(tail.1, "malformed", Status::Partial);
        check_schedule(tail, evidence, partial, false)?;
        assert!(
            rows.iter()
                .all(|row| row.subresource.namespace != SEGMENT
                    || row.subresource.indices != [1, 0, 1]),
            "legacy rows continue to require complete stingers"
        );
        Ok(())
    })
}

#[test]
fn older_music_layout_keeps_raw_envelopes_without_invented_schedule() -> TestResult {
    let object = TestHircObject::music_segment(OWNER_ID, &[], &[]);
    let bytes = source(153, 10, object, &[NONZERO], None)?;
    export(&bytes, false, HircLayout::Unsupported, |rows, _| {
        assert!(
            rows.iter()
                .all(|row| row.subresource.namespace != NAMESPACE),
            "the older bank is outside the proved schedule layout"
        );
        Ok(())
    })
}

fn check_schedule(
    record: (usize, &MediaRecord),
    evidence: &Evidence<'_>,
    schedule: Schedule,
    complete: bool,
) -> TestResult {
    for (field, label, value) in [
        ("trigger_id", "musicStingerScheduleTriggerId", TRIGGER_ID),
        ("segment_id", "musicStingerScheduleSegmentId", SEGMENT_ID),
        ("sync_play_at", "musicStingerSyncPlayAt", schedule.sync),
        ("cue_filter_hash", "musicStingerCueFilterHash", schedule.cue),
        (
            "dont_repeat_time/bits",
            "musicStingerDontRepeatBits",
            u32::from_le_bytes(schedule.repeat.to_le_bytes()),
        ),
    ] {
        evidence.word(record, field, label, value)?;
    }
    evidence.conversion(
        record,
        "dont_repeat_time/signed_value",
        "dont_repeat_time/bits",
        "musicStingerDontRepeatSigned",
    )?;
    assert_eq!(
        record
            .1
            .data
            .pointer("/payload/dont_repeat_time/signed_value"),
        Some(&json!(schedule.repeat)),
        "the repeat time keeps the original signed interpretation",
    );
    if complete {
        evidence.word(
            record,
            "segment_look_ahead",
            "musicStingerSegmentLookAhead",
            schedule.look_ahead,
        )?;
    } else {
        assert!(
            record
                .1
                .data
                .pointer("/payload/segment_look_ahead")
                .is_none(),
            "an incomplete original look-ahead word has no typed value"
        );
    }
    Ok(())
}

fn source(
    version: u32,
    kind: u8,
    original: TestHircObject,
    schedules: &[Schedule],
    final_schedule_bytes: Option<usize>,
) -> TestResult<Vec<u8>> {
    let input = bank(154, original);
    let parsed = Soundbank::parse(&input)?;
    let object = parsed
        .objects()
        .first()
        .ok_or("original synthetic music owner")?;
    let mut body = parsed
        .object_body(object)
        .get(size_of::<u32>()..)
        .ok_or("original object body")?
        .to_vec();
    let offsets = schedule_offsets(&body);
    if offsets.len() != schedules.len() {
        return Err("the synthetic marker pair must identify exactly the authored stingers".into());
    }
    for (offset, schedule) in offsets.iter().copied().zip(schedules) {
        body.get_mut(offset..offset + 16)
            .ok_or("complete fixture schedule")?
            .copy_from_slice(schedule.words().as_flattened());
    }
    if let Some(bytes) = final_schedule_bytes {
        body.truncate(offsets.last().ok_or("fixture final schedule")? + bytes);
    }
    Ok(bank(version, TestHircObject::raw(kind, OWNER_ID, body)))
}

fn schedule_offsets(body: &[u8]) -> Vec<usize> {
    let words = [TRIGGER_ID.to_le_bytes(), SEGMENT_ID.to_le_bytes()];
    body.windows(words.as_flattened().len())
        .enumerate()
        .filter_map(|(index, window)| {
            (window == words.as_flattened()).then_some(index + window.len())
        })
        .collect()
}

impl Schedule {
    const fn words(self) -> [[u8; 4]; 4] {
        [
            self.sync.to_le_bytes(),
            self.cue.to_le_bytes(),
            self.repeat.to_le_bytes(),
            self.look_ahead.to_le_bytes(),
        ]
    }
}
