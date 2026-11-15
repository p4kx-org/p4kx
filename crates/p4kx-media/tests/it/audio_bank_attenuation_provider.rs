// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::hirc_select::select_with_sources;
use p4kx_extraction::hirc_view::ObjectView;
use p4kx_extraction::{Bank, HircKind, HircLayout};
use p4kx_media::{MediaRecord, Status};
use p4kx_testkit::TestHircObject;
use serde_json::{Value, json};

use super::audio_bank_container_node_provider::fixture::{Evidence, bank, export, layout};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

const SUMMARY: &str = "audio-bank-hirc-attenuation";
const SELECTOR: &str = "audio-bank-hirc-attenuation-selector";
const CURVE: &str = "audio-bank-hirc-attenuation-curve";
const POINT: &str = "audio-bank-hirc-attenuation-point";
const RTPC: &str = "audio-bank-hirc-attenuation-rtpc";
const RTPC_CURVE: &str = "audio-bank-hirc-attenuation-rtpc-curve";
const RTPC_POINT: &str = "audio-bank-hirc-attenuation-rtpc-point";
const NONFINITE: u32 = 0x7fc0_1234;
const UNKNOWN_INTERPOLATION: u32 = 0x7264_1020;
const CONE: [u32; 5] = [
    0x42b4_0000,
    0x4334_0000,
    0xbc80_0000,
    0x40a0_0000,
    0x4140_0000,
];

#[test]
fn attenuation_cone_selectors_curves_and_rtpc_use_original_encrypted_ranges() -> TestResult {
    let source = attenuation(154, populated_body());
    export(&source, true, HircLayout::Decoded, |rows, evidence| {
        check_summary(rows, evidence)?;
        for (ordinal, expected) in [(0, -1), (1, 1), (2, -128)] {
            let selector = row(rows, SELECTOR, &[1, 0, ordinal])?;
            assert_eq!(
                selector.1.data.pointer("/payload/curve_index/value"),
                Some(&json!(expected)),
                "the signed selector retains its literal stored index"
            );
            evidence.index(
                selector.0,
                2,
                selector.1,
                "curve_index",
                "attenuationSelectorPosition",
            )?;
        }
        let curve = row(rows, CURVE, &[1, 0, 0])?;
        evidence.byte(curve, "scaling", "attenuationScaling", 0xFE)?;
        evidence.halfword(curve, "point_count", "attenuationPointCount", 2)?;
        evidence.halfword(
            row(rows, CURVE, &[1, 0, 1])?,
            "point_count",
            "attenuationPointCount",
            0,
        )?;
        check_points(rows, evidence)?;
        Ok(())
    })
}

fn check_summary(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let summary = row(rows, SUMMARY, &[1, 0])?;
    layout(summary.1, "decoded", Status::Complete);
    evidence.byte(
        summary,
        "observed/height_spread_flags",
        "heightSpreadFlags",
        0x80,
    )?;
    evidence.byte(summary, "observed/cone_flags", "coneFlags", 0x81)?;
    assert_eq!(
        summary.1.data.pointer("/payload/observed/cone_enabled"),
        Some(&json!(true)),
        "the cone layout uses bit zero"
    );
    evidence.conversion(
        summary,
        "observed/cone_enabled",
        "observed/cone_flags",
        "coneFlags",
    )?;
    let cone_fields = [
        ("observed/cone/inside_degrees_bits", "coneInsideDegreesBits"),
        (
            "observed/cone/outside_degrees_bits",
            "coneOutsideDegreesBits",
        ),
        ("observed/cone/outside_volume_bits", "coneOutsideVolumeBits"),
        ("observed/cone/low_pass_bits", "coneLowPassBits"),
        ("observed/cone/high_pass_bits", "coneHighPassBits"),
    ];
    for ((path, label), bits) in cone_fields.into_iter().zip(CONE) {
        evidence.word(summary, path, label, bits)?;
    }
    Ok(())
}

fn check_points(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let point = row(rows, POINT, &[1, 0, 0, 0])?;
    let curve = row(rows, CURVE, &[1, 0, 0])?;
    evidence.index(
        point.0,
        2,
        curve.1,
        "scaling",
        "attenuationPointCurvePosition",
    )?;
    evidence.index(point.0, 3, point.1, "from_bits", "attenuationPointPosition")?;
    evidence.word(point, "from_bits", "rtpcFromBits", NONFINITE)?;
    evidence.word(
        point,
        "interpolation",
        "rtpcInterpolation",
        UNKNOWN_INTERPOLATION,
    )?;
    assert_eq!(
        point.1.data.pointer("/payload/input"),
        Some(&Value::Null),
        "nonfinite input has exact bits and a nullable decimal projection"
    );
    let rtpc = row(rows, RTPC, &[1, 0])?;
    layout(rtpc.1, "decoded", Status::Complete);
    let rtpc_curve = row(rows, RTPC_CURVE, &[1, 0, 0])?;
    assert_eq!(
        rtpc_curve.1.data.pointer("/payload/parameter_id/value"),
        Some(&json!(0x80)),
        "the original variable-length parameter is preserved"
    );
    evidence.word(
        row(rows, RTPC_POINT, &[1, 0, 0, 0])?,
        "to_bits",
        "rtpcToBits",
        NONFINITE,
    )
}

#[test]
fn attenuation_disabled_and_other_versions_preserve_separate_dispositions() -> TestResult {
    let body = empty_body();
    for (version, expected, label) in [
        (154, HircLayout::Decoded, "decoded"),
        (153, HircLayout::Unsupported, "unsupported"),
        (155, HircLayout::Unsupported, "unsupported"),
    ] {
        let source = attenuation(version, &body);
        export(&source, false, expected, |rows, _| {
            let result = row(rows, SUMMARY, &[1, 0])?.1;
            layout(result, label, Status::Complete);
            assert!(
                result.data.pointer("/payload/observed/cone").is_none(),
                "disabled or unsupported layouts have no cone words"
            );
            assert_eq!(
                result.data.pointer("/payload/observed/cone_enabled"),
                (version == 154).then_some(&Value::Bool(false)),
                "the decoded disabled flag is distinct from an unsupported layout"
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn attenuation_truncation_and_tail_leave_the_following_object_reachable() -> TestResult {
    let body = populated_body();
    let partials = [
        body.get(..1).ok_or("height-only prefix")?.to_vec(),
        body.get(..9).ok_or("partial cone")?.to_vec(),
        body.get(..25).ok_or("partial selectors")?.to_vec(),
        body.get(..50).ok_or("partial conversion points")?.to_vec(),
        body.get(..body.len() - 1).ok_or("partial RTPC")?.to_vec(),
        maximal_conversion_count(),
        overflowing_rtpc_parameter(),
        [body, vec![0x9D]].concat(),
    ];
    for partial in partials {
        let source = attenuation(154, partial);
        export(&source, false, HircLayout::Malformed, |rows, _| {
            let summary = row(rows, SUMMARY, &[1, 0])?.1;
            layout(summary, "malformed", Status::Partial);
            assert!(
                summary
                    .data
                    .pointer("/payload/observed/checked_range")
                    .is_some(),
                "the checked source prefix is explicit"
            );
            assert!(
                summary
                    .issues
                    .iter()
                    .any(|issue| issue.code == "audio.bank.invalid-attenuation"),
                "the attenuation receiver reports the original layout error"
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn attenuation_object_view_carries_original_bytes_and_matches_the_shared_schema() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("attenuation.bnk");
    std::fs::write(&path, attenuation(154, populated_body()))?;
    let owner = Bank::from_file_observed(&path)?;
    let page = select_with_sources(&owner, Some(HircKind::Attenuation), None, 0, 1);
    let object = page.shown.first().ok_or("selected attenuation object")?;
    let raw = serde_json::to_value(object)?;
    jsonschema::validator_for(&serde_json::to_value(schemars::schema_for!(
        ObjectView<'_>
    ))?)?
    .validate(&raw)
    .map_err(|error| error.to_string())?;
    assert_eq!(
        raw.pointer("/source_fields/fields/cone/high_pass_bits/value"),
        Some(&json!(0x4140_0000)),
        "the same console and MCP view keeps the original cone word"
    );
    let rows = raw
        .pointer("/source_fields/rows")
        .and_then(Value::as_array)
        .ok_or("conversion rows")?;
    let point = rows
        .iter()
        .find(|row| row["kind"] == "point")
        .ok_or("original conversion point")?;
    let cell = point.get("from_bits").ok_or("original input source cell")?;
    assert_eq!(
        cell["value"],
        json!(NONFINITE),
        "nonfinite bits are lossless in ObjectView"
    );
    assert_eq!(
        cell["sourceSetComplete"],
        json!(true),
        "the observed file provides the complete source"
    );
    assert_eq!(
        raw.pointer("/source_fields/initial_rtpc/declared_curve_count/value"),
        Some(&json!(1)),
        "the shared initial RTPC reader supplies its declared payload"
    );
    Ok(())
}

fn attenuation(version: u32, body: impl AsRef<[u8]>) -> Vec<u8> {
    bank(version, TestHircObject::raw(14, 5, body.as_ref()))
}

fn empty_body() -> Vec<u8> {
    let mut body = vec![0x27, 0x80];
    body.extend([0xff; 19]);
    body.extend([0, 0, 0]);
    body
}

fn maximal_conversion_count() -> Vec<u8> {
    let mut body = vec![0; 2];
    body.extend([0xff; 19]);
    body.extend([1, 0xFD]);
    body.extend(u16::MAX.to_le_bytes());
    body
}

fn overflowing_rtpc_parameter() -> Vec<u8> {
    let mut body = vec![0; 2];
    body.extend([0xff; 19]);
    body.push(0);
    body.extend(1_u16.to_le_bytes());
    body.extend([0; 6]);
    body.extend([0xFF; 5]);
    body
}

fn populated_body() -> Vec<u8> {
    let mut body = vec![0x80, 0x81];
    words(&mut body, CONE);
    body.extend([0xff, 1, 0x80]);
    body.extend([0xff; 16]);
    body.extend([2, 0xfe, 2, 0]);
    words(
        &mut body,
        [
            NONFINITE,
            0,
            UNKNOWN_INTERPOLATION,
            0x8000_0000,
            0x4000_0000,
            4,
        ],
    );
    body.extend([1, 0, 0]);
    body.extend(1_u16.to_le_bytes());
    words(&mut body, [0xA710_0010]);
    body.extend([4, 8, 0x81, 0]);
    words(&mut body, [0xBB51_0005]);
    body.extend([3, 1, 0]);
    words(&mut body, [0, NONFINITE, UNKNOWN_INTERPOLATION]);
    body
}

fn words<const N: usize>(body: &mut Vec<u8>, values: [u32; N]) {
    body.extend(values.into_iter().flat_map(u32::to_le_bytes));
}
