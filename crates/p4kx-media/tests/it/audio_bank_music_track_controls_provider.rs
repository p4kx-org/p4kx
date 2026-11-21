// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{Evidence, export, layout, plain_node};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

#[path = "audio_bank_music_track_controls_fixture.rs"]
mod fixture;

use self::fixture::{append_words, clips, switch, track};

const CONTROL: &str = "audio-bank-music-track-controls";
const AUTOMATION: &str = "audio-bank-music-track-automation";
const POINT: &str = "audio-bank-music-track-automation-point";

#[test]
fn v154_track_exports_overlapping_clips_automation_and_switch_transition() -> TestResult {
    let mut body = clips();
    append_words(
        &mut body,
        [
            2,
            1,
            u32::MAX,
            2,
            0x7FC0_0101,
            0x8000_0000,
            u32::MAX,
            0x3F80_0000,
            0xBF80_0000,
            4,
            1,
            4,
            0,
        ],
    );
    body.extend(switch()?);
    body.extend((-300_i32).to_le_bytes());
    body.extend(0xFEDD_1133_u32.to_le_bytes());
    body.extend((-14_i32).to_le_bytes());
    append_words(&mut body, [0xFF22_1177, 0]);
    body.extend(1500_i32.to_le_bytes());
    body.extend(0xFC20_1122_u32.to_le_bytes());
    body.extend(i32::MIN.to_le_bytes());
    body.extend((-15_i32).to_le_bytes());
    let bytes = track(154, body);
    export(&bytes, true, HircLayout::Decoded, |rows, evidence| {
        let control = row(rows, CONTROL, &[1, 0])?;
        layout(control.1, "decoded", Status::Complete);
        evidence.byte(control, "observed/flags", "musicTrackFlags", 0xD1)?;
        evidence.word(
            control,
            "observed/subtrack_count",
            "musicTrackSubtrackCount",
            3,
        )?;
        evidence.word(
            control,
            "observed/automation/declared_count",
            "musicTrackAutomationCount",
            2,
        )?;
        signed(
            control,
            evidence,
            "observed/look_ahead",
            "musicTrackLookAheadBits",
            -15,
        )?;
        transition(control, evidence)?;
        automations(rows, evidence)?;
        legacy_clips(rows)
    })
}

fn transition(control: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    for (field, label, value) in [
        ("source/time", "musicTrackSourceTransitionTimeBits", -300),
        ("source/offset", "musicTrackSourceTransitionOffsetBits", -14),
        (
            "destination/time",
            "musicTrackDestinationTransitionTimeBits",
            1500,
        ),
        (
            "destination/offset",
            "musicTrackDestinationTransitionOffsetBits",
            i32::MIN,
        ),
    ] {
        signed(
            control,
            evidence,
            &format!("observed/transition/{field}"),
            label,
            value,
        )?;
    }
    for (field, label, value) in [
        ("source/curve", "musicTrackSourceFadeCurve", 0xFEDD_1133),
        ("sync_type", "musicTrackTransitionSyncType", 0xFF22_1177),
        ("cue_filter_hash", "musicTrackTransitionCueFilterHash", 0),
        (
            "destination/curve",
            "musicTrackDestinationFadeCurve",
            0xFC20_1122,
        ),
    ] {
        evidence.word(
            control,
            &format!("observed/transition/{field}"),
            label,
            value,
        )?;
    }
    Ok(())
}

fn signed(
    control: (usize, &MediaRecord),
    evidence: &Evidence<'_>,
    field: &str,
    label: &str,
    value: i32,
) -> TestResult {
    let bits = format!("{field}/bits");
    evidence.word(
        control,
        &bits,
        label,
        u32::from_le_bytes(value.to_le_bytes()),
    )?;
    evidence.conversion(
        control,
        &format!("{field}/signed_value"),
        &bits,
        "musicTrackSignedWord",
    )?;
    assert_eq!(
        control
            .1
            .data
            .pointer(&format!("/payload/{field}/signed_value")),
        Some(&json!(value)),
        "{field} preserves the original signed interpretation",
    );
    Ok(())
}

fn automations(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (ordinal, kind, points) in [(0, u32::MAX, 2), (1, 4, 0)] {
        let automation = row(rows, AUTOMATION, &[1, 0, ordinal])?;
        layout(automation.1, "decoded", Status::Complete);
        evidence.word(automation, "clip_index", "musicTrackAutomationClipIndex", 1)?;
        evidence.word(
            automation,
            "automation_type",
            "musicTrackAutomationType",
            kind,
        )?;
        evidence.word(
            automation,
            "points/declared_count",
            "musicTrackAutomationPointCount",
            points,
        )?;
    }
    let automation = row(rows, AUTOMATION, &[1, 0, 0])?.1;
    let point = row(rows, POINT, &[1, 0, 0, 0])?;
    layout(point.1, "decoded", Status::Complete);
    evidence.word(
        point,
        "from/bits",
        "musicTrackAutomationFromBits",
        0x7FC0_0101,
    )?;
    evidence.word(point, "to/bits", "musicTrackAutomationToBits", 0x8000_0000)?;
    evidence.word(
        point,
        "interpolation",
        "musicTrackAutomationInterpolation",
        u32::MAX,
    )?;
    evidence.index(
        point.0,
        2,
        automation,
        "clip_index",
        "musicTrackAutomationPosition",
    )?;
    assert!(
        point.1.data.pointer("/payload/from/finite_value").is_none(),
        "the original NaN has no finite interpretation"
    );
    evidence.conversion(
        point,
        "to/finite_value",
        "to/bits",
        "musicTrackAutomationFiniteValue",
    )
}

fn legacy_clips(rows: &[MediaRecord]) -> TestResult {
    const CLIP: &str = "audio-bank-music-track-clip";
    let first = row(rows, CLIP, &[1, 0, 0])?.1;
    let second = row(rows, CLIP, &[1, 0, 1])?.1;
    assert_eq!(
        first.data.pointer("/payload/play_at/finite"),
        Some(&json!(120.0)),
        "the first authored clip keeps its original placement",
    );
    assert_eq!(
        second.data.pointer("/payload/play_at/finite"),
        Some(&json!(240.0)),
        "the overlapping second clip keeps its original placement",
    );
    assert_eq!(
        second.data.pointer("/payload/begin_trim/finite"),
        Some(&json!(0.5)),
        "the second clip retains its original trim",
    );
    Ok(())
}

#[test]
fn truncated_automation_retains_complete_word_prefix_and_original_maximum_count() -> TestResult {
    let mut body = clips();
    append_words(
        &mut body,
        [1, 1, 0xEF22_1000, u32::MAX, 0x3F80_0000, 0x4000_0000],
    );
    body.extend([0xC7, 0xD8]);
    let bytes = track(154, body);
    export(&bytes, false, HircLayout::Malformed, |rows, evidence| {
        let control = row(rows, CONTROL, &[1, 0])?;
        layout(control.1, "malformed", Status::Partial);
        assert!(
            control
                .1
                .data
                .pointer("/payload/observed/look_ahead")
                .is_none()
        );
        let automation = row(rows, AUTOMATION, &[1, 0, 0])?;
        layout(automation.1, "malformed", Status::Partial);
        evidence.word(
            automation,
            "points/declared_count",
            "musicTrackAutomationPointCount",
            u32::MAX,
        )?;
        let point = row(rows, POINT, &[1, 0, 0, 0])?;
        layout(point.1, "malformed", Status::Partial);
        evidence.word(
            point,
            "from/bits",
            "musicTrackAutomationFromBits",
            0x3F80_0000,
        )?;
        evidence.word(point, "to/bits", "musicTrackAutomationToBits", 0x4000_0000)?;
        assert!(point.1.data.pointer("/payload/interpolation").is_none());
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == POINT)
                .count(),
            1
        );
        legacy_clips(rows)
    })
}

#[test]
fn truncated_switch_transition_keeps_readable_signed_time_and_curve() -> TestResult {
    let mut body = clips();
    append_words(&mut body, [0]);
    body.extend(switch()?);
    body.extend((-17_i32).to_le_bytes());
    append_words(&mut body, [0xFB00_0011]);
    body.extend([0xC5, 0xD4]);
    let bytes = track(154, body);
    export(&bytes, true, HircLayout::Malformed, |rows, evidence| {
        let control = row(rows, CONTROL, &[1, 0])?;
        layout(control.1, "malformed", Status::Partial);
        signed(
            control,
            evidence,
            "observed/transition/source/time",
            "musicTrackSourceTransitionTimeBits",
            -17,
        )?;
        evidence.word(
            control,
            "observed/transition/source/curve",
            "musicTrackSourceFadeCurve",
            0xFB00_0011,
        )?;
        assert!(
            control
                .1
                .data
                .pointer("/payload/observed/transition/source/offset")
                .is_none()
        );
        assert!(
            control
                .1
                .data
                .pointer("/payload/observed/transition/destination")
                .is_none()
        );
        assert!(
            control
                .1
                .data
                .pointer("/payload/observed/look_ahead")
                .is_none()
        );
        Ok(())
    })
}

#[test]
fn normal_track_with_no_automation_keeps_its_explicit_zeroes() -> TestResult {
    let mut body = 0_u32.to_le_bytes().to_vec();
    body.push(0);
    append_words(&mut body, [0, 0]);
    body.extend(plain_node()?);
    body.push(0);
    body.extend(0_i32.to_le_bytes());
    for (version, expected, status) in [
        (154, HircLayout::Decoded, "decoded"),
        (153, HircLayout::Unsupported, "unsupported"),
    ] {
        let bytes = track(version, body.clone());
        export(&bytes, false, expected, |rows, evidence| {
            let control = row(rows, CONTROL, &[1, 0])?;
            layout(control.1, status, Status::Complete);
            if version == 154 {
                evidence.word(
                    control,
                    "observed/automation/declared_count",
                    "musicTrackAutomationCount",
                    0,
                )?;
                evidence.byte(control, "observed/flags", "musicTrackFlags", 0)?;
                signed(
                    control,
                    evidence,
                    "observed/look_ahead",
                    "musicTrackLookAheadBits",
                    0,
                )?;
                assert!(
                    control
                        .1
                        .data
                        .pointer("/payload/observed/subtrack_count")
                        .is_none()
                );
                assert!(
                    control
                        .1
                        .data
                        .pointer("/payload/observed/transition")
                        .is_none()
                );
            } else {
                assert!(control.1.data.pointer("/payload/observed").is_none());
            }
            assert!(
                rows.iter()
                    .all(|row| row.subresource.namespace != AUTOMATION)
            );
            Ok(())
        })?;
    }
    Ok(())
}
