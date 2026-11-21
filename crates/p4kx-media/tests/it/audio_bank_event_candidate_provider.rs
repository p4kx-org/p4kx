// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use p4kx_extraction::bank::BankSource;
use p4kx_extraction::{Archive, HircKind, OpenOptions, Soundbank};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject, test_key};
use serde_json::{Value, json};

use super::support;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SUMMARY: &str = "audio-bank-hirc-event-action-resolution";
const CANDIDATE: &str = "audio-bank-hirc-event-action-candidate";
const WIDE: u64 = 130;
const MATCH: u32 = 42;
const UNIQUE: u32 = 91;
const MISSING: u32 = 99;
const WRONG_KIND: u32 = 101;

#[test]
fn event_action_resolution_streams_all_physical_candidates_and_distinguishes_wrong_kinds()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let mut bank = TestBank::new(77)
        .object(TestHircObject::event(
            1,
            &[MATCH, UNIQUE, MISSING, WRONG_KIND, MATCH],
        ))
        .object(TestHircObject::sound(MATCH, 2, 2, 0))
        .object(TestHircObject::action(UNIQUE, 0x0403, 3))
        .object(TestHircObject::sound(WRONG_KIND, 2, 2, 0));
    for _ in 0..WIDE {
        bank = bank.object(TestHircObject::action(MATCH, 0x0403, 3));
    }
    let bytes = bank.bytes();
    let original = Soundbank::parse(bytes.as_slice())?;
    let original_positions = original
        .objects()
        .iter()
        .enumerate()
        .filter(|(_, object)| object.id == MATCH)
        .map(|(index, object)| (index, object.kind == HircKind::Action))
        .collect::<Vec<_>>();
    assert_eq!(
        original_positions.len() as u64,
        WIDE + 1,
        "physical ID occurrences before export"
    );
    assert_eq!(
        original_positions[0],
        (1, false),
        "same-ID Sound is a physical nonmatch"
    );
    let archive = archive(folder.path(), &bytes, false)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export(&archive, &output, &options())?)?;
    let rows = support::records(&output, &index, "audio.jsonl")?;
    check_schema(&rows)?;
    for (ordinal, expected) in [
        (0, ("ambiguous", WIDE + 1, WIDE)),
        (1, ("resolved", 1, 1)),
        (2, ("missing", 0, 0)),
        (3, ("missing", 1, 0)),
        (4, ("ambiguous", WIDE + 1, WIDE)),
    ] {
        check_summary(&rows, ordinal, expected, true)?;
    }
    check_duplicate_candidates(&rows, &original_positions)?;
    Ok(())
}

fn check_duplicate_candidates(
    rows: &[MediaRecord],
    original_positions: &[(usize, bool)],
) -> TestResult {
    for action_ordinal in [0, 4] {
        let observed = rows
            .iter()
            .filter(|row| {
                row.subresource.namespace == CANDIDATE
                    && row.subresource.indices.get(2) == Some(&action_ordinal)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            observed.len(),
            original_positions.len(),
            "every occurrence survives the default 128 candidate limit"
        );
        for (index, (row, &(target, matches))) in
            observed.into_iter().zip(original_positions).enumerate()
        {
            assert_eq!(
                row.subresource.indices,
                [1, 0, action_ordinal, u64::try_from(index)?],
                "ordered reference candidate identity"
            );
            assert_eq!(
                row.data.pointer("/payload/target_section_ordinal"),
                Some(&json!("1")),
                "candidate uses the original HIRC section"
            );
            assert_eq!(
                row.data.pointer("/payload/target_object_ordinal"),
                Some(&json!(target.to_string())),
                "candidate uses the original object occurrence"
            );
            assert_eq!(
                row.data.pointer("/payload/action_candidate"),
                Some(&json!(matches)),
                "physical type distinguishes Action from a same-ID Sound"
            );
        }
    }
    Ok(())
}

#[test]
fn an_incomplete_hirc_keeps_observed_candidates_without_claiming_missing_or_unique() -> TestResult {
    let folder = tempfile::tempdir()?;
    let bytes = truncated_hirc()?;
    let archive = archive(folder.path(), &bytes, true)?;
    let output = folder.path().join("out");
    let diagnostic = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &diagnostic)?
    else {
        return Err("structural HIRC fault must leave diagnostic output".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = support::records(base, &index, "audio.jsonl")?;
    check_schema(&rows)?;
    check_summary(&rows, 0, ("unavailable", 1, 1), false)?;
    check_summary(&rows, 1, ("unavailable", 0, 0), false)?;
    let candidate = row_at(&rows, CANDIDATE, &[1, 0, 0, 0])?;
    check_origin(
        base,
        &rows,
        candidate,
        "/data/payload/action_candidate",
        true,
    )?;
    let summary = row_at(&rows, SUMMARY, &[1, 0, 0])?;
    check_origin(base, &rows, summary, "/data/payload/resolution", false)?;
    let retained = candidate
        .source
        .raw
        .as_ref()
        .ok_or("retained encrypted bank")?;
    assert_eq!(
        fs::read(base.join(&index.artifact_base).join(&retained.path))?,
        bytes,
        "original damaged bank survives exactly"
    );
    Ok(())
}

fn truncated_hirc() -> TestResult<Vec<u8>> {
    let mut bytes = TestBank::new(78)
        .object(TestHircObject::event(1, &[MATCH, MISSING]))
        .object(TestHircObject::action(MATCH, 0x0403, 3))
        .bytes();
    let original = Soundbank::parse(bytes.as_slice())?;
    let section = original
        .sections()
        .find(|section| section.id.0 == *b"HIRC")
        .ok_or("HIRC section")?;
    assert_eq!(
        section.body.get(..4),
        Some(2_u32.to_le_bytes().as_slice()),
        "original fixture object count"
    );
    let at = bytes
        .windows(4)
        .position(|word| word == b"HIRC")
        .ok_or("HIRC tag")?
        + 8;
    bytes
        .get_mut(at..at + 4)
        .ok_or("original count bytes")?
        .copy_from_slice(&3_u32.to_le_bytes());
    assert!(
        Soundbank::parse(bytes.as_slice()).is_err(),
        "physical third object was deliberately left unreadable"
    );
    Ok(bytes)
}

#[test]
fn duplicate_hirc_sections_retain_each_physical_candidate() -> TestResult {
    let folder = tempfile::tempdir()?;
    let foreign = TestBank::new(77)
        .object(TestHircObject::action(MATCH, 0x0403, 4))
        .bytes();
    let secondary = Soundbank::parse(foreign.as_slice())?;
    let other_hirc = secondary
        .sections()
        .find(|section| section.id.0 == *b"HIRC")
        .ok_or("second HIRC source bytes")?;
    let bytes = TestBank::new(77)
        .object(TestHircObject::event(1, &[MATCH]))
        .object(TestHircObject::action(MATCH, 0x0403, 3))
        .section(*b"HIRC", other_hirc.body)
        .bytes();
    let mut scanner = BankSource::open(Cursor::new(&bytes), u64::try_from(bytes.len())?)?;
    let mut directory = scanner.section_cursor();
    let mut seen = Vec::new();
    while let Some(section) = scanner.next_section(&mut directory) {
        let section = section?;
        if section.id().0 == *b"HIRC" {
            for object in scanner.hirc(&section)? {
                let object = object?;
                if object.id() == MATCH {
                    seen.push((object.section_ordinal(), object.ordinal()));
                }
            }
        }
    }
    assert_eq!(
        seen,
        [(1, 1), (2, 0)],
        "independent physical object receipts"
    );
    let archive = archive(folder.path(), &bytes, false)?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { index, diagnostic } =
        p4kx_media::export(&archive, &output, &options)?
    else {
        return Err("the repeated HIRC tag is independently diagnosed".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic generation")?;
    let rows = support::records(base, &index, "audio.jsonl")?;
    check_summary(&rows, 0, ("ambiguous", 2, 2), true)?;
    for (ordinal, (section, object)) in seen.into_iter().enumerate() {
        let candidate = row_at(&rows, CANDIDATE, &[1, 0, 0, u64::try_from(ordinal)?])?;
        assert_eq!(
            candidate.data.pointer("/payload/target_section_ordinal"),
            Some(&json!(section.to_string()))
        );
        assert_eq!(
            candidate.data.pointer("/payload/target_object_ordinal"),
            Some(&json!(object.to_string()))
        );
    }
    Ok(())
}

fn check_origin(
    base: &Path,
    rows: &[MediaRecord],
    row: &MediaRecord,
    field: &str,
    complete: bool,
) -> TestResult {
    let ordinal = rows
        .iter()
        .position(|entry| std::ptr::eq(entry, row))
        .ok_or("emitted row")?
        .to_string();
    for line in fs::read_to_string(base.join("field-origins.jsonl"))?.lines() {
        let receipt: Value = serde_json::from_str(line)?;
        if receipt.pointer("/scope/stream/kind") == Some(&json!("audio"))
            && receipt.pointer("/scope/ordinal") == Some(&json!(ordinal))
            && receipt.get("field") == Some(&json!(field))
        {
            assert_eq!(
                receipt.get("containsEncryptedSource"),
                Some(&json!(true)),
                "actual encrypted bank origin"
            );
            assert_eq!(
                receipt.get("sourceSetComplete"),
                Some(&json!(complete)),
                "known candidate vs incomplete inventory"
            );
            let sources = receipt
                .get("sourceUsesRefs")
                .and_then(Value::as_array)
                .ok_or("actual source receipts")?;
            assert!(!sources.is_empty(), "observed source evidence is retained");
            return Ok(());
        }
    }
    Err(format!("missing origin for {field} on audio row {ordinal}").into())
}

fn check_summary(
    rows: &[MediaRecord],
    ordinal: u64,
    expected: (&str, u64, u64),
    complete: bool,
) -> TestResult {
    let row = row_at(rows, SUMMARY, &[1, 0, ordinal])?;
    let (status, observed, matching) = expected;
    for (field, value) in [
        ("resolution", json!(status)),
        ("observed_candidates", json!(observed.to_string())),
        ("matching_candidates", json!(matching.to_string())),
        ("candidates_complete", json!(complete)),
    ] {
        assert_eq!(
            row.data
                .get("payload")
                .and_then(|payload| payload.get(field)),
            Some(&value),
            "action {ordinal}, {field}"
        );
    }
    Ok(())
}

fn row_at<'rows>(
    rows: &'rows [MediaRecord],
    namespace: &str,
    indices: &[u64],
) -> TestResult<&'rows MediaRecord> {
    rows.iter()
        .find(|row| row.subresource.namespace == namespace && row.subresource.indices == indices)
        .ok_or_else(|| format!("missing {namespace} at {indices:?}").into())
}

fn check_schema(rows: &[MediaRecord]) -> TestResult {
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
    ExportOptions {
        kinds: vec![MediaKind::Audio],
        ..ExportOptions::default()
    }
}

fn archive(folder: &Path, bytes: &[u8], encrypted: bool) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    let entry = TestEntry::stored("Data\\Sounds\\wwise\\candidate.bnk", bytes);
    let entry = if encrypted {
        TestEntry::zstd("Data\\Sounds\\wwise\\candidate.bnk", bytes).encrypted()
    } else {
        entry
    };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
