// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::HircLayout;
use p4kx_media::{ExportOptions, ExportOutcome, MediaRecord, Status};
use serde_json::{Value, json};

use super::audio_bank_state_provider::{
    PATH, archive, check_retained, inline_bank_with_rtpc, inline_chunk, options, row, source_span,
    verify_layout,
};
use super::support;
use super::support::audio_provider::{Origins, Owner, TestResult};

const SUMMARY: &str = "audio-bank-hirc-sound-rtpc";
const CURVE: &str = "audio-bank-hirc-sound-rtpc-curve";
const POINT: &str = "audio-bank-hirc-sound-rtpc-point";
const NAN_BITS: u32 = 0x7FC1_2345;
const NEGATIVE_ZERO: u32 = 0x8000_0000;
const UNKNOWN_INTERPOLATION: u32 = 0xF901_0017;
const RTPC_ID: u32 = 0xFA01_8888;
const CURVE_ID: u32 = 0x33EA_88FF;

#[test]
fn sound_rtpc_rows_keep_exact_authored_values_and_provenance() -> TestResult {
    let folder = tempfile::tempdir()?;
    let original = rtpc_bytes();
    let source = inline_bank_with_rtpc(&original);
    verify_layout(&source, HircLayout::Decoded)?;
    let archive = archive(folder.path(), &source)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let base = output.join(&index.artifact_base);
    let (_, summary) = row(&rows, SUMMARY, &[1, 0])?;
    assert_summary(summary, "decoded", 2, 2)?;
    assert_eq!(
        source_span(&source, &summary.data, "/payload/checked_range")?,
        original,
        "checked RTPC extent follows original State and covers only RTPC"
    );
    check_retained(&base, summary, &source)?;

    let (_, first) = row(&rows, CURVE, &[1, 0, 0])?;
    let (_, second) = row(&rows, CURVE, &[1, 0, 1])?;
    assert_curve(&source, first, 1)?;
    assert_curve(&source, second, 0)?;
    let (position, point) = row(&rows, POINT, &[1, 0, 0, 0])?;
    assert_point(&source, point)?;
    assert_word_origin(&base, position, point)?;
    validate_rows(&rows)
}

#[test]
fn malformed_rtpc_keeps_complete_curve_prefix_and_checked_inline_state() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut original = rtpc_bytes();
    original[0..2].copy_from_slice(&3_u16.to_le_bytes());
    append_curve(&mut original, u16::MAX);
    let source = inline_bank_with_rtpc(&original);
    verify_layout(&source, HircLayout::Malformed)?;
    let archive = archive(folder.path(), &source)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("malformed bank produces diagnostic generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic base")?;
    let rows = support::records(base, &index, "audio.jsonl")?;
    let (_, summary) = row(&rows, SUMMARY, &[1, 0])?;
    assert_summary(summary, "malformed", 3, 2)?;
    assert_eq!(
        summary.status,
        Status::Partial,
        "RTPC source fault remains explicit"
    );
    assert_eq!(
        source_span(&source, &summary.data, "/payload/checked_range")?,
        &original[..rtpc_bytes().len()],
        "truncated final curve excluded from checked prefix"
    );
    let (_, state) = row(&rows, "audio-bank-hirc-inline-state", &[1, 0])?;
    assert_eq!(
        source_span(&source, &state.data, "/payload/checked_range")?,
        inline_chunk(),
        "earlier State and exact extent survive"
    );
    assert_eq!(
        state.status,
        Status::Complete,
        "independently checked State extent"
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r.subresource.namespace == CURVE)
            .count(),
        2,
        "no unproved final curve record"
    );
    let (_, point) = row(&rows, POINT, &[1, 0, 0, 0])?;
    assert_point(&source, point)?;
    check_retained(base, summary, &source)?;
    validate_rows(&rows)
}

#[test]
fn a_short_rtpc_count_has_an_explicit_unavailable_summary() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = inline_bank_with_rtpc(&[1]);
    verify_layout(&source, HircLayout::Malformed)?;
    let archive = archive(folder.path(), &source)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("short count leaves diagnostic generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic base")?;
    let rows = support::records(base, &index, "audio.jsonl")?;
    let (_, summary) = row(&rows, SUMMARY, &[1, 0])?;
    assert_eq!(
        summary.data.pointer("/payload/layout"),
        Some(&json!("unavailable")),
        "the original count did not fit"
    );
    assert!(
        summary.data.pointer("/payload/curve_count").is_none(),
        "no fabricated count"
    );
    assert!(
        rows.iter()
            .all(|row| !matches!(row.subresource.namespace.as_str(), CURVE | POINT)),
        "unreadable count emits no curve or point rows"
    );
    validate_rows(&rows)
}

fn assert_summary(record: &MediaRecord, layout: &str, declared: u16, checked: u16) -> TestResult {
    let payload = record.data.get("payload").ok_or("RTPC payload")?;
    assert_eq!(payload.get("layout"), Some(&json!(layout)), "RTPC layout");
    assert_eq!(
        payload.get("version"),
        Some(&json!(154)),
        "observed BKHD version"
    );
    assert_eq!(
        payload.pointer("/curve_count/value"),
        Some(&json!(declared)),
        "original count"
    );
    assert_eq!(
        payload.get("checked_curves"),
        Some(&json!(checked)),
        "complete physical prefix"
    );
    Ok(())
}

fn assert_curve(source: &[u8], record: &MediaRecord, points: u16) -> TestResult {
    for (field, value, expected) in [
        ("rtpc_id", json!(RTPC_ID), RTPC_ID.to_le_bytes().to_vec()),
        ("rtpc_type", json!(0xF1), vec![0xF1]),
        ("accumulation", json!(0xE2), vec![0xE2]),
        ("parameter_id", json!(135), vec![0x81, 0x07]),
        ("curve_id", json!(CURVE_ID), CURVE_ID.to_le_bytes().to_vec()),
        ("scaling", json!(0xD3), vec![0xD3]),
        ("point_count", json!(points), points.to_le_bytes().to_vec()),
    ] {
        assert_eq!(
            record.data.pointer(&format!("/payload/{field}/value")),
            Some(&value),
            "authored {field}"
        );
        assert_eq!(
            source_span(source, &record.data, &format!("/payload/{field}/range"))?,
            expected,
            "original {field} bytes"
        );
    }
    Ok(())
}

fn assert_point(source: &[u8], record: &MediaRecord) -> TestResult {
    for (field, bits) in [
        ("from_bits", NAN_BITS),
        ("to_bits", NEGATIVE_ZERO),
        ("interpolation", UNKNOWN_INTERPOLATION),
    ] {
        assert_eq!(
            record.data.pointer(&format!("/payload/{field}/value")),
            Some(&json!(bits)),
            "raw {field} word"
        );
        assert_eq!(
            source_span(source, &record.data, &format!("/payload/{field}/range"))?,
            bits.to_le_bytes(),
            "original {field} range"
        );
    }
    assert_eq!(
        record.data.pointer("/payload/input"),
        Some(&Value::Null),
        "nonfinite value retained as original bits"
    );
    assert_eq!(
        record
            .data
            .pointer("/payload/output")
            .and_then(Value::as_f64)
            .map(f64::to_bits),
        Some((-0.0_f64).to_bits()),
        "finite negative zero interpretation"
    );
    Ok(())
}

fn assert_word_origin(base: &Path, position: usize, record: &MediaRecord) -> TestResult {
    let origins = Origins::read(base)?;
    let owner = Owner {
        path: PATH,
        entry: 0,
        parent: None,
        stored: "encrypted",
        metadata: false,
        payload: true,
    };
    let range = record
        .data
        .pointer("/payload/from_bits/range")
        .ok_or("point source range")?;
    let indices = json!([
        range.get("start").ok_or("range start")?,
        range.get("end_exclusive").ok_or("range end")?
    ]);
    for field in ["/data/payload/from_bits/value", "/data/payload/input"] {
        let datum = origins.field("records", position, field, true)?;
        let uses = origins.require_owner(datum, owner)?;
        assert!(
            uses.iter().any(
                |used| used.pointer("/location/field") == Some(&json!("rtpcFromBits"))
                    && used.pointer("/location/indices") == Some(&indices)
            ),
            "datum receipt resolves to the encrypted original word"
        );
    }
    Ok(())
}

fn validate_rows(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows
        .iter()
        .filter(|row| row.subresource.namespace.contains("sound-rtpc"))
    {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn rtpc_bytes() -> Vec<u8> {
    let mut bytes = 2_u16.to_le_bytes().to_vec();
    append_curve(&mut bytes, 1);
    bytes.extend(
        [NAN_BITS, NEGATIVE_ZERO, UNKNOWN_INTERPOLATION]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    append_curve(&mut bytes, 0);
    bytes
}

fn append_curve(bytes: &mut Vec<u8>, points: u16) {
    bytes.extend(RTPC_ID.to_le_bytes());
    bytes.extend([0xF1, 0xE2, 0x81, 0x07]);
    bytes.extend(CURVE_ID.to_le_bytes());
    bytes.push(0xD3);
    bytes.extend(points.to_le_bytes());
}
