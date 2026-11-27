// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{Archive, HircLayout, OpenOptions, Soundbank};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::{Value, json};

use super::support;
use super::support::audio_provider::{Origins, Owner, TestResult};

pub(super) const PATH: &str = "state.bnk";
const STANDALONE: &str = "audio-bank-hirc-state";
const STANDALONE_PROPERTY: &str = "audio-bank-hirc-state-property";
const INLINE: &str = "audio-bank-hirc-inline-state";
const BINDING: &str = "audio-bank-hirc-inline-state-binding";
const GROUP: &str = "audio-bank-hirc-inline-state-group";
const ENTRY: &str = "audio-bank-hirc-inline-state-entry";
const PROPERTY: &str = "audio-bank-hirc-inline-state-property";
const UNKNOWN_PROPERTY: u16 = 0xF302;
const NAN_BITS: u32 = 0x7FC1_2345;

#[test]
fn standalone_state_rows_preserve_bits_ranges_and_opaque_tail() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = standalone(145, &standalone_body());
    verify_layout(&bytes, HircLayout::Decoded)?;
    let archive = archive(folder.path(), &bytes)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let base = output.join(&index.artifact_base);
    let (_, summary) = row(&rows, STANDALONE, &[1, 0])?;
    assert_eq!(
        summary.data.pointer("/payload/layout"),
        Some(&json!("decoded")),
        "proved standalone layout"
    );
    assert_eq!(
        summary.data.pointer("/payload/property_count/value"),
        Some(&json!(2)),
        "authored count"
    );
    assert_eq!(
        source_span(&bytes, &summary.data, "/payload/trailing_range")?,
        &[0xA5],
        "opaque tail is recoverable"
    );
    check_retained(&base, summary, &bytes)?;
    let (position, first) = row(&rows, STANDALONE_PROPERTY, &[1, 0, 0])?;
    check_property(&bytes, first, UNKNOWN_PROPERTY, NAN_BITS)?;
    assert_eq!(
        first.data.pointer("/payload/value"),
        Some(&Value::Null),
        "NaN is represented through exact bits"
    );
    check_origin(
        &base,
        position,
        "/data/payload/value_bits/value",
        "stateValueBits",
    )?;
    let (_, second) = row(&rows, STANDALONE_PROPERTY, &[1, 0, 1])?;
    check_property(&bytes, second, UNKNOWN_PROPERTY, 0x8000_0000)?;
    validate(&rows)
}

#[test]
fn inline_states_preserve_order_and_raw_words_in_a_checked_sound_region() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = inline_bank_with_rtpc(&[0, 0]);
    verify_layout(&bytes, HircLayout::Decoded)?;
    let archive = archive(folder.path(), &bytes)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let base = output.join(&index.artifact_base);
    let (_, summary) = row(&rows, INLINE, &[1, 0])?;
    assert_eq!(
        summary.data.pointer("/payload/layout"),
        Some(&json!("decoded")),
        "checked inline State chunk"
    );
    assert_eq!(
        source_span(&bytes, &summary.data, "/payload/checked_range")?,
        inline_chunk(),
        "State extent excludes following RTPC word"
    );
    let (position, binding) = row(&rows, BINDING, &[1, 0, 0])?;
    assert_eq!(
        binding.data.pointer("/payload/property_id/value"),
        Some(&json!(258)),
        "multi-byte property binding"
    );
    assert_eq!(
        source_span(&bytes, &binding.data, "/payload/accumulation/range")?,
        &[0xF0],
        "raw accumulation byte"
    );
    assert_eq!(
        source_span(&bytes, &binding.data, "/payload/in_db/range")?,
        &[0x81],
        "raw in-dB byte"
    );
    check_origin(&base, position, "/data/payload/in_db/value", "stateInDb")?;
    let (_, group) = row(&rows, GROUP, &[1, 0, 0])?;
    assert_eq!(
        group.data.pointer("/payload/synchronization/value"),
        Some(&json!(0xE1)),
        "unknown synchronization bits"
    );
    let (_, member) = row(&rows, ENTRY, &[1, 0, 0, 0])?;
    assert_eq!(
        member.data.pointer("/payload/state_id/value"),
        Some(&json!(55)),
        "authored State member"
    );
    let (_, value) = row(&rows, PROPERTY, &[1, 0, 0, 0, 0])?;
    check_property(&bytes, value, UNKNOWN_PROPERTY, NAN_BITS)?;
    check_retained(&base, summary, &bytes)?;
    validate(&rows)
}

#[test]
fn unproved_standalone_state_keeps_raw_bytes_without_property_rows() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = standalone(154, &standalone_body());
    verify_layout(&bytes, HircLayout::Unsupported)?;
    let archive = archive(folder.path(), &bytes)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let base = output.join(&index.artifact_base);
    let (_, summary) = row(&rows, STANDALONE, &[1, 0])?;
    assert_eq!(
        summary.status,
        Status::Complete,
        "explicit unsupported coverage retains the physical source"
    );
    assert_eq!(
        summary.data.pointer("/payload/layout"),
        Some(&json!("unsupported")),
        "standalone type 1 v154 is unproved"
    );
    assert!(
        rows.iter()
            .all(|row| row.subresource.namespace != STANDALONE_PROPERTY),
        "unsupported bytes are not interpreted"
    );
    check_retained(&base, summary, &bytes)?;
    validate(&rows)
}

#[test]
fn later_rtpc_failure_keeps_checked_inline_state_rows_in_diagnostics() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = inline_bank_with_rtpc(&[1]);
    verify_layout(&bytes, HircLayout::Malformed)?;
    let archive = archive(folder.path(), &bytes)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("original malformed RTPC must mark diagnostic generation".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic base")?;
    let rows = support::records(base, &index, "audio.jsonl")?;
    let (_, sound) = row(&rows, "audio-bank-hirc-sound", &[1, 0])?;
    assert_eq!(
        sound.status,
        Status::Partial,
        "following RTPC read failure is explicit"
    );
    let (_, state) = row(&rows, INLINE, &[1, 0])?;
    assert_eq!(
        state.status,
        Status::Complete,
        "earlier checked State region is complete"
    );
    let (_, value) = row(&rows, PROPERTY, &[1, 0, 0, 0, 0])?;
    check_property(&bytes, value, UNKNOWN_PROPERTY, NAN_BITS)?;
    check_retained(base, state, &bytes)?;
    validate(&rows)
}

fn standalone_body() -> Vec<u8> {
    let mut payload = 2_u16.to_le_bytes().to_vec();
    payload.extend([UNKNOWN_PROPERTY; 2].into_iter().flat_map(u16::to_le_bytes));
    payload.extend(
        [NAN_BITS, 0x8000_0000]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    payload.push(0xA5);
    payload
}

fn standalone(version: u32, body: &[u8]) -> Vec<u8> {
    TestBank::new(7)
        .version(version)
        .object(TestHircObject::raw(1, 100, body))
        .bytes()
}

pub(super) fn inline_chunk() -> Vec<u8> {
    let mut body = vec![1, 0x82, 0x02, 0xF0, 0x81, 1];
    body.extend(0xF0_0102_u32.to_le_bytes());
    body.extend([0xE1, 1]);
    body.extend(55_u32.to_le_bytes());
    body.extend(1_u16.to_le_bytes());
    body.extend(UNKNOWN_PROPERTY.to_le_bytes());
    body.extend(NAN_BITS.to_le_bytes());
    body
}

pub(super) fn inline_bank_with_rtpc(rtpc: &[u8]) -> Vec<u8> {
    let mut body = 0x0004_0001_u32.to_le_bytes().to_vec();
    body.push(2);
    body.extend([1_u32, 2, 0].into_iter().flat_map(u32::to_le_bytes));
    body.push(0);
    body.extend([0, 0, 0, 0]);
    body.extend([0_u32, 15].into_iter().flat_map(u32::to_le_bytes));
    body.extend([0, 0, 0, 0, 0]);
    body.extend(0_u32.to_le_bytes());
    body.extend([0, 1, 0, 0, 3, 0x0C]);
    body.extend(inline_chunk());
    body.extend(rtpc);
    TestBank::new(7)
        .object(TestHircObject::raw(2, 77, body))
        .bytes()
}

pub(super) fn verify_layout(bytes: &[u8], expected: HircLayout) -> TestResult {
    let bank = Soundbank::parse(bytes)?;
    let object = bank
        .objects()
        .first()
        .ok_or("original physical HIRC object")?;
    assert_eq!(
        object.layout(),
        expected,
        "independent original slice layout before export"
    );
    Ok(())
}

fn check_property(source: &[u8], record: &MediaRecord, id: u16, bits: u32) -> TestResult {
    assert_eq!(
        record.data.pointer("/payload/property_id/value"),
        Some(&json!(id)),
        "original property ID"
    );
    assert_eq!(
        record.data.pointer("/payload/value_bits/value"),
        Some(&json!(bits)),
        "exact binary32 word"
    );
    assert_eq!(
        source_span(source, &record.data, "/payload/property_id/range")?,
        &id.to_le_bytes(),
        "property span addresses original bytes"
    );
    assert_eq!(
        source_span(source, &record.data, "/payload/value_bits/range")?,
        &bits.to_le_bytes(),
        "value span addresses original bytes"
    );
    Ok(())
}

pub(super) fn source_span<'a>(
    source: &'a [u8],
    value: &Value,
    pointer: &str,
) -> TestResult<&'a [u8]> {
    let range = value.pointer(pointer).ok_or("source range")?;
    let number = |key| -> TestResult<usize> {
        Ok(range
            .get(key)
            .and_then(Value::as_str)
            .ok_or("source range position")?
            .parse()?)
    };
    source
        .get(number("start")?..number("end_exclusive")?)
        .ok_or_else(|| "bank-relative extent".into())
}

pub(super) fn check_retained(base: &Path, record: &MediaRecord, original: &[u8]) -> TestResult {
    let raw = record
        .source
        .raw
        .as_ref()
        .ok_or("verified original bank raw artifact")?;
    assert_eq!(
        fs::read(base.join(&raw.path))?,
        original,
        "original encrypted source retained exactly"
    );
    Ok(())
}

fn check_origin(base: &Path, position: usize, field: &str, source_field: &str) -> TestResult {
    let evidence = Origins::read(base)?;
    let field = evidence.field("records", position, field, true)?;
    let owner = Owner {
        path: PATH,
        entry: 0,
        parent: None,
        stored: "encrypted",
        metadata: false,
        payload: true,
    };
    let uses = evidence.require_owner(field, owner)?;
    assert!(
        uses.iter()
            .any(|used| used.pointer("/location/field") == Some(&json!(source_field))),
        "value records the exact physical field use"
    );
    Ok(())
}

pub(super) fn row<'a>(
    rows: &'a [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'a MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn validate(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows
        .iter()
        .filter(|row| row.subresource.namespace.contains("state"))
    {
        let value = serde_json::to_value(row)?;
        validator
            .validate(&value)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn options() -> ExportOptions {
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}

pub(super) fn archive(folder: &Path, bytes: &[u8]) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(PATH, bytes).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
