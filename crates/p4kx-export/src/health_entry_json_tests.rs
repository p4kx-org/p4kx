// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read as _;

use p4kx_extraction::{
    Archive, Entry, InputScope, SourceEncryptionState, SourceLocation, SourceUseRole,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{EntryJson, JsonEvent};
use crate::catalog_read::original::{EntryCapture, EntryRead, EntryReceipt, with_entry_capture};
use crate::{CatalogError, ExportAccountingError};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ORIGINAL: &str =
    r#"  {"bu\u0079":9007199254740993.2500,"buy":9007199254740993.2500,"lines":[-0.00e+00,null]} "#;
const PRICE: &str = "9007199254740993.2500";

fn archive(bytes: &str) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.json", bytes))
        .entry(TestEntry::stored("encrypted.json", bytes).encrypted())
        .write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn read(
    archive: &Archive,
    entry: &Entry,
    capture: &EntryCapture<'_>,
) -> Result<EntryRead, CatalogError> {
    let receipt = EntryReceipt::outer(capture, archive, entry)?;
    EntryRead::read_all(receipt, archive, entry, None)?
        .map_err(|_| ExportAccountingError::ConflictingSourceDatum.into())
}

fn assert_original(archive: &Archive, entry: &Entry, expected: &str) -> TestResult {
    let mut decoded = String::new();
    archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_string(&mut decoded)?;
    assert_eq!(decoded, expected, "independent decoded archive bytes");
    Ok(())
}

fn assert_number_uses(
    root: EntryJson<'_>,
    encryption: SourceEncryptionState,
) -> Result<(), CatalogError> {
    let mut events = root.walk();
    assert!(
        matches!(events.next().transpose()?, Some(JsonEvent::Object)),
        "root object"
    );
    for ((start, price), escaped) in ORIGINAL.match_indices(PRICE).zip([true, false]) {
        let name = next_name(&mut events)?;
        assert_eq!(
            name.text().map_err(CatalogError::Json)?.as_deref(),
            Some("buy"),
            "decoded key"
        );
        assert_eq!(
            name.raw().contains('\\'),
            escaped,
            "distinct original key spellings"
        );
        let key_use = name.observe(SourceUseRole::Selector)?;
        assert_eq!(
            key_use.role(),
            SourceUseRole::Selector,
            "actual key token role"
        );
        let value = next_value(&mut events)?;
        assert_eq!(value.number(), Some(price), "exact original number token");
        assert_number_origin(value, start, price.len(), encryption)?;
    }
    assert_eq!(
        next_name(&mut events)?.raw(),
        "\"lines\"",
        "actual array name"
    );
    assert!(
        matches!(events.next().transpose()?, Some(JsonEvent::Array)),
        "authored array"
    );
    assert_eq!(
        next_value(&mut events)?.number(),
        Some("-0.00e+00"),
        "signed source number"
    );
    assert_eq!(next_value(&mut events)?.raw(), "null", "explicit null slot");
    assert!(
        matches!(events.next().transpose()?, Some(JsonEvent::End(_))),
        "exact array end"
    );
    assert!(
        matches!(events.next().transpose()?, Some(JsonEvent::End(_))),
        "exact object end"
    );
    assert!(events.next().is_none(), "all original tokens visited");
    Ok(())
}

fn next_name<'read>(
    events: &mut impl Iterator<Item = Result<JsonEvent<'read>, CatalogError>>,
) -> Result<EntryJson<'read>, CatalogError> {
    match events.next().transpose()? {
        Some(JsonEvent::Name(name)) => Ok(name),
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

fn next_value<'read>(
    events: &mut impl Iterator<Item = Result<JsonEvent<'read>, CatalogError>>,
) -> Result<EntryJson<'read>, CatalogError> {
    match events.next().transpose()? {
        Some(JsonEvent::Value(value)) => Ok(value),
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}

fn assert_number_origin(
    value: EntryJson<'_>,
    start: usize,
    len: usize,
    encryption: SourceEncryptionState,
) -> Result<(), CatalogError> {
    let used = value.observe(SourceUseRole::Payload)?;
    let start = u64::try_from(start).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    let len = u64::try_from(len).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
    assert_eq!(
        used.description().location(),
        &SourceLocation::Subresource {
            namespace: "archiveJson".to_owned(),
            indices: vec![start, len],
            field: "$token".to_owned(),
        },
        "each equal price addresses its own actual decoded byte range"
    );
    assert_eq!(
        used.description().scope(),
        InputScope::Payload,
        "payload domain"
    );
    assert_eq!(
        used.encryption(),
        encryption,
        "actual original Entry encryption"
    );
    Ok(())
}

#[test]
fn duplicate_tokens_keep_exact_ranges_and_actual_entry_encryption() -> TestResult {
    let (_folder, archive) = archive(ORIGINAL)?;
    for (name, encryption) in [
        ("plain.json", SourceEncryptionState::Unencrypted),
        ("encrypted.json", SourceEncryptionState::Encrypted),
    ] {
        let entry = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == name)
            .ok_or("original physical Entry")?;
        assert_original(&archive, entry, ORIGINAL)?;
        with_entry_capture(&archive, |capture| {
            let read = read(&archive, entry, capture)?;
            let parsed = EntryJson::parse(&read, capture)?.expect("valid complete JSON");
            assert_number_uses(parsed, encryption)
        })?;
    }
    Ok(())
}

#[test]
fn deep_source_children_keep_every_event_and_complete_outer_range() -> TestResult {
    let depth = 257;
    let nested = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    let original = format!(r#"{{"unknown":{nested},"price":1e+2}}"#);
    let (_folder, archive) = archive(&original)?;
    let entry = archive.entries().first().ok_or("original physical Entry")?;
    assert_original(&archive, entry, &original)?;
    with_entry_capture(&archive, |capture| {
        let read = read(&archive, entry, capture)?;
        let parsed = EntryJson::parse(&read, capture)?.expect("validated deep JSON");
        let mut events = parsed.walk();
        assert!(
            matches!(events.next().transpose()?, Some(JsonEvent::Object)),
            "root object"
        );
        assert_eq!(
            next_name(&mut events)?.raw(),
            "\"unknown\"",
            "actual deep member"
        );
        for _ in 0..depth {
            assert!(
                matches!(events.next().transpose()?, Some(JsonEvent::Array)),
                "each original array"
            );
        }
        assert_eq!(
            next_value(&mut events)?.number(),
            Some("0"),
            "deepest original value"
        );
        let mut outer = None;
        for _ in 0..depth {
            let Some(JsonEvent::End(token)) = events.next().transpose()? else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            outer = Some(token.raw());
        }
        assert_eq!(outer, Some(nested.as_str()), "the full outer source range");
        assert_eq!(
            next_name(&mut events)?.raw(),
            "\"price\"",
            "next authored member"
        );
        assert_eq!(
            next_value(&mut events)?.number(),
            Some("1e+2"),
            "following number"
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn rejects_bounded_reads_and_foreign_captures_before_binding() -> TestResult {
    let (_folder, archive) = archive(ORIGINAL)?;
    let entry = archive.entries().first().ok_or("original physical Entry")?;
    assert_original(&archive, entry, ORIGINAL)?;
    with_entry_capture(&archive, |capture| {
        let receipt = EntryReceipt::outer(capture, &archive, entry)?;
        let bounded = EntryRead::read(receipt, &archive, entry, entry.uncompressed_size())?
            .expect("bounded reader successfully obtains the declared bytes");
        assert_eq!(bounded.bytes(), ORIGINAL.as_bytes(), "equal decoded bytes");
        assert!(
            matches!(
                EntryJson::parse(&bounded, capture),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "bounded success lacks a checked decoded end"
        );
        let complete = read(&archive, entry, capture)?;
        assert!(
            EntryJson::parse(&complete, capture)?.is_ok(),
            "the actual complete read parses in the same capture"
        );
        with_entry_capture(&archive, |foreign| {
            assert!(
                matches!(
                    EntryJson::parse(&complete, foreign),
                    Err(CatalogError::Accounting(
                        ExportAccountingError::ConflictingSourceDatum
                    ))
                ),
                "same Archive and equal bytes cannot substitute a second inventory"
            );
            let sources = foreign.frame().sources().freeze()?;
            assert!(
                sources.uses().is_empty(),
                "no foreign token use was retained"
            );
            Ok(())
        })
    })?;
    Ok(())
}
