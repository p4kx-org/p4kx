// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{BankError, HircLayout, Soundbank};
use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{
    Evidence, export, export_raw_bank, layout, source,
};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

const BLEND_TYPE: u8 = 9;
const OBJECT_ID: u32 = 0x9B00_1122;
const LAYER_ID: u32 = 0x9AA0_3300;
const CHILD_A: u32 = 0xA137_5511;
const CHILD_B: u32 = 0xB157_6612;
const CROSSFADE_ID: u32 = 0xCC12_2030;
const CURVE_RTPC: u32 = 0xDABC_0011;
const CURVE_ID: u32 = 0xE152_4020;
const SUMMARY: &str = "audio-bank-hirc-blend-container";
const LAYER: &str = "audio-bank-hirc-blend-layer";
const ASSOCIATION: &str = "audio-bank-hirc-blend-association";
const POINT: &str = "audio-bank-hirc-blend-point";
const LAYER_RTPC: &str = "audio-bank-hirc-blend-layer-rtpc";
const LAYER_RTPC_CURVE: &str = "audio-bank-hirc-blend-layer-rtpc-curve";
const LAYER_RTPC_POINT: &str = "audio-bank-hirc-blend-layer-rtpc-point";

#[test]
fn version_154_blend_retains_every_authored_layer_curve_and_position() -> TestResult {
    let mut continuation = children();
    append_words(&mut continuation, [2, LAYER_ID]);
    append_initial_rtpc(&mut continuation);
    append_crossfade(&mut continuation, CROSSFADE_ID, 0xFE, 2);
    append_words(
        &mut continuation,
        [
            CHILD_B,
            2,
            0x7FC1_1234,
            0x8000_0000,
            u32::MAX,
            0xBF80_0000,
            0x4040_0000,
            4,
            CHILD_B,
            0,
            LAYER_ID,
        ],
    );
    continuation.extend(0_u16.to_le_bytes());
    append_crossfade(&mut continuation, 0, 0, 0);
    continuation.push(0xE0);
    let bytes = source(BLEND_TYPE, OBJECT_ID, 154, &continuation)?;
    export(&bytes, true, HircLayout::Decoded, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "decoded", Status::Complete);
        evidence.word(
            summary,
            "observed/layers/declared_count",
            "blendLayerCount",
            2,
        )?;
        evidence.byte(
            summary,
            "observed/continuous_validation",
            "blendContinuousValidation",
            0xE0,
        )?;
        layers(rows, evidence)?;
        associations(rows, evidence)?;
        initial_rtpc(rows, evidence)
    })
}

fn layers(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (ordinal, id, kind, count) in [(0, CROSSFADE_ID, 0xFE, 2), (1, 0, 0, 0)] {
        let layer = row(rows, LAYER, &[1, 0, ordinal])?;
        layout(layer.1, "decoded", Status::Complete);
        evidence.word(layer, "layer_id", "blendLayerId", LAYER_ID)?;
        evidence.word(layer, "crossfade_rtpc_id", "blendCrossfadeRtpcId", id)?;
        evidence.byte(layer, "crossfade_rtpc_type", "blendCrossfadeRtpcType", kind)?;
        evidence.word(
            layer,
            "associations/declared_count",
            "blendAssociationCount",
            count,
        )?;
    }
    let original = row(rows, "audio-bank-hirc-container-child", &[1, 0, 0])?.1;
    assert_eq!(
        original.data.pointer("/payload/child_id"),
        Some(&json!(CHILD_A)),
        "the original child order is separate from Layer associations"
    );
    Ok(())
}

fn associations(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (ordinal, count) in [(0, 2), (1, 0)] {
        let association = row(rows, ASSOCIATION, &[1, 0, 0, ordinal])?;
        layout(association.1, "decoded", Status::Complete);
        evidence.word(association, "child_id", "blendAssociatedChildId", CHILD_B)?;
        evidence.word(
            association,
            "points/declared_count",
            "blendPointCount",
            count,
        )?;
    }
    for (ordinal, from, to, interpolation) in [
        (0, 0x7FC1_1234, 0x8000_0000, u32::MAX),
        (1, 0xBF80_0000, 0x4040_0000, 4),
    ] {
        let point = row(rows, POINT, &[1, 0, 0, 0, ordinal])?;
        layout(point.1, "decoded", Status::Complete);
        evidence.word(point, "from/bits", "blendPointFromBits", from)?;
        evidence.word(point, "to/bits", "blendPointToBits", to)?;
        evidence.word(
            point,
            "interpolation",
            "blendPointInterpolation",
            interpolation,
        )?;
        let association = row(rows, ASSOCIATION, &[1, 0, 0, 0])?.1;
        evidence.index(
            point.0,
            3,
            association,
            "child_id",
            "blendAssociationPosition",
        )?;
        evidence.conversion(point, "to/finite_value", "to/bits", "blendFinitePointValue")?;
    }
    Ok(())
}

fn initial_rtpc(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let summary = row(rows, LAYER_RTPC, &[1, 0, 0])?;
    layout(summary.1, "decoded", Status::Complete);
    evidence.halfword(summary, "curve_count", "rtpcCurveCount", 1)?;
    let curve = row(rows, LAYER_RTPC_CURVE, &[1, 0, 0, 0])?;
    evidence.word(curve, "rtpc_id", "rtpcId", CURVE_RTPC)?;
    evidence.word(curve, "curve_id", "rtpcCurveId", CURVE_ID)?;
    let layer = row(rows, LAYER, &[1, 0, 0])?.1;
    evidence.index(curve.0, 2, layer, "layer_id", "blendLayerPosition")?;
    let point = row(rows, LAYER_RTPC_POINT, &[1, 0, 0, 0, 0])?;
    evidence.word(point, "from_bits", "rtpcFromBits", 0x3F00_0000)?;
    evidence.word(point, "to_bits", "rtpcToBits", 0x4000_0000)
}

#[test]
fn truncated_association_keeps_only_the_available_original_point_words() -> TestResult {
    let mut continuation = children();
    append_words(&mut continuation, [1, LAYER_ID]);
    continuation.extend(0_u16.to_le_bytes());
    append_crossfade(&mut continuation, CROSSFADE_ID, 0xFE, 1);
    append_words(
        &mut continuation,
        [CHILD_B, u32::MAX, 0x7F80_0000, 0xBF80_0000],
    );
    continuation.extend([0xCD, 0xE5]);
    let bytes = source(BLEND_TYPE, OBJECT_ID, 154, &continuation)?;
    export(&bytes, false, HircLayout::Malformed, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/continuous_validation")
                .is_none()
        );
        let rtpc = row(rows, LAYER_RTPC, &[1, 0, 0])?;
        layout(rtpc.1, "decoded", Status::Complete);
        let association = row(rows, ASSOCIATION, &[1, 0, 0, 0])?;
        layout(association.1, "malformed", Status::Partial);
        evidence.word(
            association,
            "points/declared_count",
            "blendPointCount",
            u32::MAX,
        )?;
        let point = row(rows, POINT, &[1, 0, 0, 0, 0])?;
        layout(point.1, "malformed", Status::Partial);
        evidence.word(point, "from/bits", "blendPointFromBits", 0x7F80_0000)?;
        evidence.word(point, "to/bits", "blendPointToBits", 0xBF80_0000)?;
        assert!(point.1.data.pointer("/payload/interpolation").is_none());
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == POINT)
                .count(),
            1
        );
        Ok(())
    })
}

#[test]
fn empty_blend_and_older_layout_have_explicit_separate_coverage() -> TestResult {
    let mut continuation = children();
    append_words(&mut continuation, [0]);
    continuation.push(0);
    for (version, expected, state) in [
        (154, HircLayout::Decoded, "decoded"),
        (153, HircLayout::Unsupported, "unsupported"),
    ] {
        let bytes = source(BLEND_TYPE, OBJECT_ID, version, &continuation)?;
        export(&bytes, false, expected, |rows, evidence| {
            let summary = row(rows, SUMMARY, &[1, 0])?;
            layout(summary.1, state, Status::Complete);
            if version == 154 {
                evidence.word(
                    summary,
                    "observed/layers/declared_count",
                    "blendLayerCount",
                    0,
                )?;
                evidence.byte(
                    summary,
                    "observed/continuous_validation",
                    "blendContinuousValidation",
                    0,
                )?;
            } else {
                assert!(summary.1.data.pointer("/payload/observed").is_none());
            }
            assert!(rows.iter().all(|row| row.subresource.namespace != LAYER));
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn custom_version_marker_keeps_the_original_header_and_raw_objects() -> TestResult {
    const CUSTOM: u32 = 0x8000_009A;
    let mut continuation = children();
    append_words(&mut continuation, [0]);
    continuation.push(0);
    let bytes = source(BLEND_TYPE, OBJECT_ID, CUSTOM, &continuation)?;
    assert!(
        matches!(Soundbank::parse(&bytes), Err(BankError::UnsupportedVersion { version }) if version == CUSTOM),
        "the custom marker is part of the original version"
    );
    export_raw_bank(&bytes, false, |rows, _| {
        let bank = row(rows, "audio-bank", &[0])?.1;
        assert_eq!(bank.data.pointer("/payload/version"), Some(&json!(CUSTOM)));
        assert_eq!(
            bank.data.pointer("/payload/version_supported"),
            Some(&json!(false))
        );
        assert!(rows.iter().all(|row| {
            ![SUMMARY, LAYER, ASSOCIATION, POINT].contains(&row.subresource.namespace.as_str())
        }));
        Ok(())
    })
}

fn children() -> Vec<u8> {
    [2, CHILD_A, CHILD_B]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn append_words(tail: &mut Vec<u8>, words: impl IntoIterator<Item = u32>) {
    tail.extend(words.into_iter().flat_map(u32::to_le_bytes));
}

fn append_crossfade(tail: &mut Vec<u8>, rtpc_id: u32, kind: u8, associations: u32) {
    tail.extend(rtpc_id.to_le_bytes());
    tail.push(kind);
    tail.extend(associations.to_le_bytes());
}

fn append_initial_rtpc(tail: &mut Vec<u8>) {
    tail.extend(1_u16.to_le_bytes());
    tail.extend(CURVE_RTPC.to_le_bytes());
    tail.extend([0xFD, 0xF9, 0x81, 0x02]);
    tail.extend(CURVE_ID.to_le_bytes());
    tail.push(0xF8);
    tail.extend(1_u16.to_le_bytes());
    append_words(tail, [0x3F00_0000, 0x4000_0000, 4]);
}
