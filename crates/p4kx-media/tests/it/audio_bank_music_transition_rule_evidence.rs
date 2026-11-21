// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::super::audio_bank_container_node_provider::fixture::{Evidence, layout};
use super::super::audio_bank_state_provider::row;
use super::super::support::audio_provider::TestResult;
use super::fixture::{DESTINATION_IDS, OBJECT_ID, SOURCE_IDS};
use super::{ID, RULE};

pub(super) fn rule_words(first: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    source_words(first, evidence)?;
    destination_words(first, evidence)?;
    evidence.byte(first, "allocation_flag", "musicTransitionAllocationFlag", 0)?;
    assert!(
        first.1.data.pointer("/payload/transition_object").is_none(),
        "the original zero allocation flag leaves no object"
    );
    Ok(())
}

fn source_words(first: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    signed(
        first,
        evidence,
        "source/fade/time",
        "musicTransitionSourceTimeBits",
        -400,
    )?;
    signed(
        first,
        evidence,
        "source/fade/offset",
        "musicTransitionSourceOffsetBits",
        -7,
    )?;
    for (field, label, value) in [
        (
            "source/fade/curve",
            "musicTransitionSourceCurve",
            0xFC00_1100,
        ),
        (
            "source/sync_type",
            "musicTransitionSourceSyncType",
            0xF100_1000,
        ),
        (
            "source/cue_filter_hash",
            "musicTransitionSourceCueFilterHash",
            0xDA00_1133,
        ),
    ] {
        evidence.word(first, field, label, value)?;
    }
    evidence.byte(
        first,
        "source/play_post_exit",
        "musicTransitionSourcePlayPostExit",
        0xE2,
    )
}

fn destination_words(first: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    signed(
        first,
        evidence,
        "destination/fade/time",
        "musicTransitionDestinationTimeBits",
        900,
    )?;
    signed(
        first,
        evidence,
        "destination/fade/offset",
        "musicTransitionDestinationOffsetBits",
        i32::MIN,
    )?;
    for (field, label, value) in [
        (
            "destination/fade/curve",
            "musicTransitionDestinationCurve",
            0xFB00_2200,
        ),
        (
            "destination/cue_filter_hash",
            "musicTransitionDestinationCueFilterHash",
            0,
        ),
        (
            "destination/jump_to_id",
            "musicTransitionJumpToId",
            0xD222_5511,
        ),
    ] {
        evidence.word(first, field, label, value)?;
    }
    evidence.halfword(
        first,
        "destination/jump_to_type",
        "musicTransitionJumpToType",
        0xFAC1,
    )?;
    evidence.halfword(
        first,
        "destination/entry_type",
        "musicTransitionEntryType",
        0xFD11,
    )?;
    evidence.byte(
        first,
        "destination/play_pre_entry",
        "musicTransitionDestinationPlayPreEntry",
        0xD1,
    )?;
    evidence.byte(
        first,
        "destination/match_source_cue_name",
        "musicTransitionMatchSourceCueName",
        0xE2,
    )
}

pub(super) fn reference_order(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let rule = row(rows, RULE, &[1, 0, 0])?.1;
    for (side, name, ids) in [
        (0, "source", SOURCE_IDS.as_slice()),
        (1, "destination", DESTINATION_IDS.as_slice()),
    ] {
        for (ordinal, id) in ids.iter().copied().enumerate() {
            let record = row(rows, ID, &[1, 0, 0, side, u64::try_from(ordinal)?])?;
            evidence.word(record, "id", "musicTransitionReferenceId", id)?;
            let count = if side == 0 {
                "source_ids/declared_count"
            } else {
                "destination_ids/declared_count"
            };
            evidence.index(record.0, 3, rule, count, "musicTransitionIdListPosition")?;
            assert_eq!(
                record.1.data.pointer("/payload/list"),
                Some(&json!(name)),
                "the authored source and destination lists have separate identities"
            );
        }
    }
    Ok(())
}

pub(super) fn object_words(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let allocated = row(rows, RULE, &[1, 0, 1])?;
    layout(allocated.1, "decoded", Status::Complete);
    evidence.byte(
        allocated,
        "allocation_flag",
        "musicTransitionAllocationFlag",
        0xA0,
    )?;
    evidence.word(
        allocated,
        "transition_object/segment_id",
        "musicTransitionObjectSegmentId",
        OBJECT_ID,
    )?;
    signed(
        allocated,
        evidence,
        "transition_object/fade_in/time",
        "musicTransitionObjectFadeInTimeBits",
        -21,
    )?;
    signed(
        allocated,
        evidence,
        "transition_object/fade_out/offset",
        "musicTransitionObjectFadeOutOffsetBits",
        -44,
    )?;
    evidence.byte(
        allocated,
        "transition_object/play_pre_entry",
        "musicTransitionObjectPlayPreEntry",
        0xD3,
    )?;
    evidence.byte(
        allocated,
        "transition_object/play_post_exit",
        "musicTransitionObjectPlayPostExit",
        0xE4,
    )
}

pub(super) fn signed(
    record: (usize, &MediaRecord),
    evidence: &Evidence<'_>,
    field: &str,
    label: &str,
    value: i32,
) -> TestResult {
    let bits = format!("{field}/bits");
    evidence.word(
        record,
        &bits,
        label,
        u32::from_le_bytes(value.to_le_bytes()),
    )?;
    evidence.conversion(
        record,
        &format!("{field}/signed_value"),
        &bits,
        "musicTransitionSignedWord",
    )?;
    assert_eq!(
        record
            .1
            .data
            .pointer(&format!("/payload/{field}/signed_value")),
        Some(&json!(value)),
        "{field} retains the signed original word"
    );
    Ok(())
}
