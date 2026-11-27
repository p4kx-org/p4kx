// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Cursor;
use std::path::Path;

use p4kx_extraction::bank::BankSource;
use p4kx_extraction::{Archive, HircValue, OpenOptions, Soundbank};
use p4kx_media::{ExportOptions, ExportOutcome, Limits, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::{Value, json};

use super::support;
use super::support::audio_provider::{Origins, Owner, TestResult, read_rows};

const PATH: &str = "local.bnk";
const SUMMARY: &str = "audio-bank-hirc-sound-media-resolution";
const CANDIDATE: &str = "audio-bank-hirc-sound-media-candidate";
const DUPLICATE: u32 = 42;
const PREFETCH: u32 = 91;
const MISSING: u32 = 99;

#[test]
fn authored_sound_storage_resolves_physical_didx_candidates_in_order() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = complete_bank()?;
    let archive = archive(folder.path(), &bytes)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    let base = output.join(&index.artifact_base);
    assert_summary(&rows, &[3, 0], "ambiguous", 2, true)?;
    assert_summary(&rows, &[3, 1], "resolved", 1, true)?;
    assert_summary(&rows, &[3, 2], "missing", 0, true)?;
    assert!(
        rows.iter().all(|row| row.subresource.namespace != SUMMARY
            || matches!(row.subresource.indices.as_slice(), [3, 0..=2])),
        "streamed and external Sources have no authored local DIDX reference"
    );
    assert_candidate(&rows, &[3, 0, 0], &[1, 0])?;
    assert_candidate(&rows, &[3, 0, 1], &[1, 1])?;
    let candidate = assert_candidate(&rows, &[3, 1, 0], &[1, 2])?;
    let data = row(&rows, "audio-bank-media", &[1, 2])?;
    assert_eq!(
        data.data.pointer("/payload/data/prefetch"),
        Some(&json!(true)),
        "linked DIDX row describes actual prefetch extent"
    );
    let origins = Origins::read(&base)?;
    let receipt = origins.field(
        "records",
        candidate.0,
        "/data/payload/target_triplet_ordinal",
        true,
    )?;
    let owner = Owner {
        path: PATH,
        entry: 0,
        parent: None,
        stored: "encrypted",
        metadata: false,
        payload: true,
    };
    let uses = origins.require_owner(receipt, owner)?;
    check_selector_bytes(&uses, &bytes)?;
    validate_reference_rows(&rows)
}

#[test]
fn damaged_didx_keeps_exact_observed_candidates_without_absence_or_uniqueness() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = damaged_bank()?;
    let archive = archive(folder.path(), &bytes)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("DIDX failure must publish diagnostic records".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = support::records(base, &index, "audio.jsonl")?;
    assert_summary(&rows, &[1, 0], "unavailable", 1, false)?;
    assert_summary(&rows, &[1, 1], "unavailable", 0, false)?;
    assert_candidate(&rows, &[1, 0, 0], &[2, 0])?;
    let ordinal = row_with_ordinal(&rows, SUMMARY, &[1, 0])?.0.to_string();
    let origins: Vec<Value> = read_rows(base.join("field-origins.jsonl"))?;
    let status = origins
        .iter()
        .find(|origin| {
            origin.pointer("/scope/ordinal") == Some(&json!(ordinal))
                && origin.pointer("/scope/stream/kind") == Some(&json!("audio"))
                && origin.get("field") == Some(&json!("/data/payload/resolution"))
        })
        .ok_or("immediate unavailable status provenance")?;
    assert_eq!(
        status.get("containsEncryptedSource"),
        Some(&json!(true)),
        "known bank contributor is encrypted"
    );
    assert_eq!(
        status.get("sourceSetComplete"),
        Some(&json!(false)),
        "unread candidate space remains incomplete"
    );
    validate_reference_rows(&rows)
}

#[test]
fn a_proven_didx_identity_is_independent_of_missing_data() -> TestResult {
    let folder = tempfile::tempdir()?;
    let triplet = [PREFETCH, 0, 12]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let bytes = TestBank::new(7)
        .object(TestHircObject::sound(1, PREFETCH, 1, 12))
        .section(*b"DIDX", triplet)
        .bytes();
    let mut source = BankSource::open(Cursor::new(bytes.as_slice()), u64::try_from(bytes.len())?)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    assert!(
        sections.iter().all(|section| section.id().0 != *b"DATA"),
        "original fixture has no DATA"
    );
    let didx = sections
        .iter()
        .find(|section| section.id().0 == *b"DIDX")
        .ok_or("original DIDX")?;
    let entry = source.didx(didx)?.next().ok_or("one original triplet")??;
    assert_eq!(entry.media_id(), PREFETCH, "physical media identity exists");
    let archive = archive(folder.path(), &bytes)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    assert_summary(&rows, &[1, 0], "resolved", 1, true)?;
    assert_candidate(&rows, &[1, 0, 0], &[2, 0])?;
    let data = row(&rows, "audio-bank-media", &[2, 0])?;
    assert_eq!(
        data.data.pointer("/payload/data/status"),
        Some(&json!("missing")),
        "DIDX identity does not establish DATA"
    );
    Ok(())
}

fn check_selector_bytes(uses: &[&Value], bytes: &[u8]) -> TestResult {
    let prefetch = PREFETCH.to_le_bytes();
    let plugin = 0x0004_0001_u32.to_le_bytes();
    for (field, expected) in [
        ("soundSourceId", prefetch.as_slice()),
        ("soundStorage", &[1]),
        ("soundPluginId", plugin.as_slice()),
        ("mediaId", prefetch.as_slice()),
    ] {
        let observation = uses
            .iter()
            .find(|used| used.pointer("/location/field") == Some(&json!(field)))
            .ok_or_else(|| format!("{field} contributes to the selected triplet"))?;
        let start = observation
            .pointer("/location/indices/0")
            .and_then(Value::as_str)
            .ok_or("actual selector byte position")?
            .parse::<usize>()?;
        assert_eq!(
            bytes.get(start..start + expected.len()),
            Some(expected),
            "{field} cites the actual original bytes"
        );
    }
    Ok(())
}

fn complete_bank() -> TestResult<Vec<u8>> {
    let mut prefix = b"RIFF".to_vec();
    prefix.extend_from_slice(&80_u32.to_le_bytes());
    prefix.extend_from_slice(b"WAVE");
    let bytes = TestBank::new(7)
        .wem(DUPLICATE, b"PLUG")
        .wem(DUPLICATE, b"PLUG")
        .wem(PREFETCH, prefix)
        .object(TestHircObject::sound(1, DUPLICATE, 0, 4))
        .object(TestHircObject::sound(2, PREFETCH, 1, 12))
        .object(TestHircObject::sound(3, MISSING, 0, 4))
        .object(TestHircObject::sound(4, DUPLICATE, 2, 0))
        .object(external_sound())
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let source_ids = bank
        .objects()
        .iter()
        .filter_map(|object| match &object.value {
            HircValue::Sound(source) => Some(source.source_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        source_ids,
        [DUPLICATE, PREFETCH, MISSING, DUPLICATE, DUPLICATE],
        "independently decoded authored source IDs"
    );
    let mut source = BankSource::open(Cursor::new(bytes.as_slice()), u64::try_from(bytes.len())?)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let didx = sections
        .iter()
        .find(|section| section.id().0 == *b"DIDX")
        .ok_or("original DIDX")?;
    let data = sections
        .iter()
        .find(|section| section.id().0 == *b"DATA")
        .ok_or("original DATA")?;
    let mut entries = source.didx(didx)?;
    let triplets = entries.by_ref().collect::<Result<Vec<_>, _>>()?;
    let [one, two, prefetch] = triplets.as_slice() else {
        return Err("three original physical triplets".into());
    };
    assert_eq!(
        [one.media_id(), two.media_id(), prefetch.media_id()],
        [DUPLICATE, DUPLICATE, PREFETCH],
        "physical ID order, including duplicates"
    );
    assert!(
        entries.inspect_media(prefetch, data)?.prefetch,
        "fixture really declares a partial RIFF"
    );
    Ok(bytes)
}

fn damaged_bank() -> TestResult<Vec<u8>> {
    let mut triplet = [DUPLICATE, 0, 4]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    triplet.push(0);
    let bytes = TestBank::new(7)
        .object(TestHircObject::sound(1, DUPLICATE, 0, 4))
        .object(TestHircObject::sound(2, MISSING, 1, 4))
        .section(*b"DIDX", triplet)
        .section(*b"DATA", b"PLUG")
        .bytes();
    let mut source = BankSource::open(Cursor::new(bytes.as_slice()), u64::try_from(bytes.len())?)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let didx = sections
        .iter()
        .find(|section| section.id().0 == *b"DIDX")
        .ok_or("original DIDX")?;
    let mut entries = source.didx(didx)?;
    let prefix = entries.next().ok_or("readable triplet")??;
    assert_eq!(
        prefix.media_id(),
        DUPLICATE,
        "known physical candidate before damage"
    );
    assert!(
        entries.next().is_some_and(|entry| entry.is_err()),
        "remaining DIDX declaration is unreadable"
    );
    Ok(bytes)
}

fn external_sound() -> TestHircObject {
    let mut body = 0x0008_0001_u32.to_le_bytes().to_vec();
    body.push(0);
    body.extend_from_slice(&DUPLICATE.to_le_bytes());
    body.extend_from_slice(&[0; 9]);
    TestHircObject::raw(2, 5, body)
}

fn assert_summary(
    rows: &[MediaRecord],
    indices: &[u64],
    state: &str,
    count: u64,
    complete: bool,
) -> TestResult {
    let row = row(rows, SUMMARY, indices)?;
    for (field, expected) in [
        ("resolution", json!(state)),
        ("observed_candidates", json!(count.to_string())),
        ("candidates_complete", json!(complete)),
    ] {
        assert_eq!(
            row.data
                .get("payload")
                .and_then(|payload| payload.get(field)),
            Some(&expected),
            "summary {indices:?}: {field}"
        );
    }
    Ok(())
}

fn assert_candidate<'rows>(
    rows: &'rows [MediaRecord],
    indices: &[u64],
    target: &[u64; 2],
) -> TestResult<(usize, &'rows MediaRecord)> {
    let (ordinal, record) = row_with_ordinal(rows, CANDIDATE, indices)?;
    for (field, value) in [
        ("target_section_ordinal", target[0]),
        ("target_triplet_ordinal", target[1]),
    ] {
        assert_eq!(
            record
                .data
                .get("payload")
                .and_then(|payload| payload.get(field)),
            Some(&json!(value.to_string())),
            "original DIDX target {field}"
        );
    }
    Ok((ordinal, record))
}

fn row<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<&'rows MediaRecord> {
    Ok(row_with_ordinal(rows, namespace, indices)?.1)
}

fn row_with_ordinal<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<(usize, &'rows MediaRecord)> {
    rows.iter()
        .enumerate()
        .find(|(_, row)| {
            row.subresource.namespace == namespace && row.subresource.indices == indices
        })
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn validate_reference_rows(rows: &[MediaRecord]) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in rows
        .iter()
        .filter(|row| matches!(row.subresource.namespace.as_str(), SUMMARY | CANDIDATE))
    {
        let json: Value = serde_json::to_value(row)?;
        validator
            .validate(&json)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn options() -> ExportOptions {
    let mut limits = Limits::default();
    limits.reference_candidates = 1;
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        limits,
        ..ExportOptions::default()
    }
}

fn archive(folder: &Path, bytes: &[u8]) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(PATH, bytes).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
