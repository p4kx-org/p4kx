// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{Evidence, export, layout, source};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

const OBJECT_ID: u32 = 0xF551_2015;
const CHILD_A: u32 = 0xA011_2233;
const CHILD_B: u32 = 0xB022_3344;
const RANDOM_SEQUENCE_TYPE: u8 = 5;
const SUMMARY: &str = "audio-bank-hirc-random-sequence";
const ITEM: &str = "audio-bank-hirc-random-sequence-playlist-item";
const CONTAINER: &str = "audio-bank-hirc-container";
const WEIGHTS: [i32; 3] = [i32::MIN, -2, 0];
const PLAY_IDS: [u32; 3] = [CHILD_B, CHILD_A, CHILD_B];

#[test]
fn v154_sequence_preserves_timing_flags_and_ordered_signed_weights() -> TestResult {
    let mut tail = heading();
    tail.extend(3_u16.to_le_bytes());
    for (id, weight) in PLAY_IDS.into_iter().zip(WEIGHTS) {
        tail.extend(id.to_le_bytes());
        tail.extend(weight.to_le_bytes());
    }
    let bytes = source(RANDOM_SEQUENCE_TYPE, OBJECT_ID, 154, &tail)?;
    export(&bytes, true, HircLayout::Decoded, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "decoded", Status::Complete);
        heading_rows(summary, evidence)?;
        evidence.halfword(
            summary,
            "observed/playlist_count",
            "randomSequencePlaylistCount",
            3,
        )?;
        playlist_rows(rows, evidence)?;
        let legacy = row(rows, CONTAINER, &[1, 0])?.1;
        assert_eq!(
            legacy.data.pointer("/payload/container_kind"),
            Some(&json!("sequence")),
            "legacy mode interpretation is preserved"
        );
        Ok(())
    })
}

fn heading_rows(summary: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    for (path, label, value) in [
        ("observed/loop_count", "randomSequenceLoopCount", 0),
        ("observed/loop_minimum", "randomSequenceLoopMinimum", 3),
        (
            "observed/loop_maximum",
            "randomSequenceLoopMaximum",
            u16::MAX,
        ),
        (
            "observed/avoid_repeat_count",
            "randomSequenceAvoidRepeatCount",
            17,
        ),
    ] {
        evidence.halfword(summary, path, label, value)?;
    }
    for (path, label, value) in [
        (
            "observed/transition_mode",
            "randomSequenceTransitionMode",
            0xFE,
        ),
        ("observed/random_mode", "randomSequenceRandomMode", 0xFD),
        ("observed/mode", "randomSequenceMode", 1),
        ("observed/flags", "randomSequenceFlags", 0xE7),
    ] {
        evidence.byte(summary, path, label, value)?;
    }
    transition_rows(summary, evidence)
}

fn transition_rows(summary: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    for (path, label, value) in [
        (
            "observed/transition_time/bits",
            "randomSequenceTransitionTime",
            0x7FC1_2345,
        ),
        (
            "observed/transition_minimum/bits",
            "randomSequenceTransitionMinimum",
            0x8000_0000,
        ),
        (
            "observed/transition_maximum/bits",
            "randomSequenceTransitionMaximum",
            0x7F80_0000,
        ),
    ] {
        evidence.word(summary, path, label, value)?;
    }
    evidence.conversion(
        summary,
        "observed/transition_minimum/finite_value",
        "observed/transition_minimum/bits",
        "randomSequenceFiniteTransition",
    )?;
    assert!(
        summary
            .1
            .data
            .pointer("/payload/observed/transition_time/finite_value")
            .is_none(),
        "the original NaN has no finite projection"
    );
    assert!(
        summary
            .1
            .data
            .pointer("/payload/observed/transition_maximum/finite_value")
            .is_none(),
        "the original infinity has no finite projection"
    );
    Ok(())
}

fn playlist_rows(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (ordinal, (id, weight)) in PLAY_IDS.into_iter().zip(WEIGHTS).enumerate() {
        let item = row(rows, ITEM, &[1, 0, u64::try_from(ordinal)?])?;
        layout(item.1, "decoded", Status::Complete);
        evidence.word(item, "play_id", "randomSequencePlayId", id)?;
        evidence.word(
            item,
            "weight_bits",
            "randomSequenceWeightBits",
            u32::from_le_bytes(weight.to_le_bytes()),
        )?;
        evidence.conversion(item, "weight", "weight_bits", "randomSequenceSignedWeight")?;
        assert_eq!(
            item.1.data.pointer("/payload/weight"),
            Some(&json!(weight)),
            "the authored i32 interpretation preserves the sign"
        );
    }
    Ok(())
}

#[test]
fn maximal_authored_playlist_count_preserves_only_the_physical_prefix() -> TestResult {
    let mut tail = heading();
    tail.extend(u16::MAX.to_le_bytes());
    tail.extend(CHILD_A.to_le_bytes());
    tail.extend([0xFE, 0xFF]);
    let bytes = source(RANDOM_SEQUENCE_TYPE, OBJECT_ID, 154, &tail)?;
    export(&bytes, false, HircLayout::Malformed, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.halfword(
            summary,
            "observed/playlist_count",
            "randomSequencePlaylistCount",
            u16::MAX,
        )?;
        let item = row(rows, ITEM, &[1, 0, 0])?;
        layout(item.1, "malformed", Status::Partial);
        evidence.word(item, "play_id", "randomSequencePlayId", CHILD_A)?;
        for field in ["weight", "weight_bits"] {
            assert!(
                item.1.data.pointer(&format!("/payload/{field}")).is_none(),
                "the original weight word was truncated"
            );
        }
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == ITEM)
                .count(),
            1,
            "no output is manufactured from the unbounded declaration"
        );
        Ok(())
    })
}

#[test]
fn short_decision_header_keeps_the_original_loop_prefix() -> TestResult {
    let bytes = source(RANDOM_SEQUENCE_TYPE, OBJECT_ID, 154, &[3, 0, 0xAF])?;
    export(&bytes, true, HircLayout::Malformed, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.halfword(summary, "observed/loop_count", "randomSequenceLoopCount", 3)?;
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/loop_minimum")
                .is_none(),
            "one source byte cannot supply a u16 loop modifier"
        );
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/playlist_count")
                .is_none(),
            "the playlist is never reached"
        );
        Ok(())
    })
}

#[test]
fn empty_playlist_and_unsupported_container_version_are_explicit() -> TestResult {
    let mut tail = heading();
    tail.extend(0_u16.to_le_bytes());
    for (version, status, expected) in [
        (154, HircLayout::Decoded, "decoded"),
        (153, HircLayout::Unsupported, "unsupported"),
    ] {
        let bytes = source(RANDOM_SEQUENCE_TYPE, OBJECT_ID, version, &tail)?;
        export(&bytes, false, status, |rows, evidence| {
            let summary = row(rows, SUMMARY, &[1, 0])?;
            layout(summary.1, expected, Status::Complete);
            if version == 154 {
                evidence.halfword(
                    summary,
                    "observed/playlist_count",
                    "randomSequencePlaylistCount",
                    0,
                )?;
            } else {
                assert!(
                    summary.1.data.pointer("/payload/observed").is_none(),
                    "the older layout is outside the proved coverage"
                );
            }
            assert!(
                rows.iter().all(|row| row.subresource.namespace != ITEM),
                "empty and unsupported paths emit no playlist rows"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn heading() -> Vec<u8> {
    let mut tail = [0_u16, 3, u16::MAX]
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    tail.extend(
        [0x7FC1_2345_u32, 0x8000_0000, 0x7F80_0000]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    tail.extend(17_u16.to_le_bytes());
    tail.extend([0xFE, 0xFD, 1, 0xE7]);
    tail.extend([2, CHILD_A, CHILD_B].into_iter().flat_map(u32::to_le_bytes));
    tail
}
