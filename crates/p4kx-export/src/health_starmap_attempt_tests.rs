// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::with_entry_capture;
use crate::ExportAccountingError;
use crate::export_ledger::{ExportLedger, SourceDisposition};

const PATH: &str = "Data\\ObjectContainers\\PU\\system\\alpha\\alphasystem.socpak";
const BYTES: &[u8] = b"actual malformed container bytes";
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn original() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("padding.txt", "ignored"))
        .entry(TestEntry::stored(PATH, BYTES).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let entry = archive.entries().get(1).ok_or("original outer Entry")?;
    assert_eq!(entry.path(), PATH, "actual original directory path");
    assert!(
        entry.is_encrypted(),
        "the original outer payload is encrypted"
    );
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    assert_eq!(
        bytes, BYTES,
        "independent original plaintext bytes before producer"
    );
    assert!(
        archive.open_container(entry).is_err(),
        "the actual original bytes are not a container"
    );
    Ok((folder, archive))
}

#[test]
fn failed_outer_starmap_open_records_payload_attempt_without_tainting_directory_path() -> TestResult
{
    let (_folder, archive) = original()?;
    let entry = archive.entries().get(1).ok_or("actual outer Entry")?;
    let (outcome, ledger, path, inventory) = with_entry_capture(&archive, |capture| {
        let mut ledger = ExportLedger::default();
        let outcome = crate::starmap_read::main_xml(&archive, entry, capture, &mut ledger)?;
        let source = ledger
            .entries
            .first()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let sources = capture.frame().sources().clone();
        let path = source.original()?.path_fact()?.into_cell(&sources);
        let inventory = sources.freeze()?;
        Ok((outcome, ledger, path, inventory))
    })?;
    assert!(outcome.is_err(), "the actual original container open fails");
    let [source] = ledger.entries.as_slice() else {
        return Err("one actual failed outer operation".into());
    };
    assert_eq!(source.entry_index, Some(1), "exact original outer position");
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "actual open failure"
    );
    let path = serde_json::to_value(path)?;
    assert_eq!(
        path.get("value"),
        Some(&json!(PATH)),
        "unchanged original directory path"
    );
    assert_eq!(
        path.get("containsEncryptedSource"),
        Some(&json!(false)),
        "leaf payload encryption cannot taint directory metadata"
    );
    attempted(&serde_json::to_value(inventory)?)
}

fn attempted(inventory: &Value) -> TestResult {
    let used = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("actual source uses")?;
    let attempts: Vec<_> = used
        .iter()
        .filter(|used| used.get("role") == Some(&json!("attempted")))
        .collect();
    assert_eq!(
        attempts.len(),
        1,
        "the real outer open has one actual payload Attempted-use"
    );
    let used = attempts.first().ok_or("actual outer read attempt")?;
    assert_eq!(
        used.get("scope"),
        Some(&json!("payload")),
        "the real read consumes payload"
    );
    assert_eq!(
        used.get("effectiveEncryption"),
        Some(&json!("encrypted")),
        "the actual encrypted payload attempt remains visible separately"
    );
    assert_eq!(
        used.pointer("/location/namespace"),
        Some(&json!("archiveEntryRead")),
        "format-owned read operation"
    );
    assert_eq!(
        used.pointer("/location/indices"),
        Some(&json!([])),
        "the admitted Entry owns the content location"
    );
    assert_eq!(
        used.pointer("/location/field"),
        Some(&json!("content")),
        "the actual failed content read"
    );
    attempted_source(used, inventory)
}

fn attempted_source(used: &Value, inventory: &Value) -> TestResult {
    let reference = used
        .get("source")
        .and_then(Value::as_str)
        .ok_or("actual attempted source")?;
    let rows = inventory
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("actual physical sources")?;
    let source = rows
        .iter()
        .find(|source| source.get("id").and_then(Value::as_str) == Some(reference))
        .ok_or("attempted source row")?;
    assert_eq!(
        source.get("entryIndex"),
        Some(&json!(1)),
        "the actual original outer Entry supplies the attempt"
    );
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!(PATH)),
        "no inner or unrelated Entry is invented"
    );
    Ok(())
}

#[test]
fn foreign_archive_is_refused_before_starmap_attempt_or_source_registration() -> TestResult {
    let (_folder, archive) = original()?;
    let (_foreign_folder, foreign) = original()?;
    let entry = foreign
        .entries()
        .get(1)
        .ok_or("genuine foreign outer Entry")?;
    let (outcome, ledger, inventory) = with_entry_capture(&archive, |capture| {
        let mut ledger = ExportLedger::default();
        let outcome = crate::starmap_read::main_xml(&foreign, entry, capture, &mut ledger);
        Ok((outcome, ledger, capture.frame().sources().freeze()?))
    })?;
    assert!(outcome.is_err(), "genuine foreign root refuses admission");
    assert!(
        ledger.entries.is_empty(),
        "no foreign Entry operation is recorded"
    );
    let inventory = serde_json::to_value(inventory)?;
    assert_eq!(
        inventory.get("sources"),
        Some(&json!([])),
        "foreign admission cannot register source rows"
    );
    assert_eq!(
        inventory.get("uses"),
        Some(&json!([])),
        "foreign admission cannot register attempted uses"
    );
    Ok(())
}
