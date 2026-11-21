// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, InputScope, SourceInput, SourceLocation, SourceUseRole, SourceUses, SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{HealthTextCapture, HealthTextCell};
use crate::catalog_read::original::OriginalDatum;
use crate::{CatalogError, ExportAccountingError, HealthMetadata};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    _folder: tempfile::TempDir,
    archive: Archive,
}

fn fixture() -> TestResult<Fixture> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("text-origins.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/plain.txt", b"same".as_slice()))
        .entry(TestEntry::stored("Data/encrypted.txt", b"same".as_slice()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok(Fixture {
        _folder: folder,
        archive,
    })
}

#[test]
fn shared_atom_keeps_plain_occurrence_visible_without_encrypted_backtaint() -> TestResult {
    let fixture = fixture()?;
    let plain = original(&fixture.archive, "plain.txt")?;
    let encrypted = original(&fixture.archive, "encrypted.txt")?;
    assert_eq!(
        plain, encrypted,
        "independent original reads retain equal exact text"
    );
    let sources = SourceUses::new();
    let capture = HealthTextCapture::new();
    let plain = capture.capture(fact(&sources, &fixture.archive, "plain.txt", &plain)?)?;
    let encrypted = capture.capture(fact(
        &sources,
        &fixture.archive,
        "encrypted.txt",
        &encrypted,
    )?)?;
    let sealed = capture.seal(&sources, None, &[&plain, &encrypted])?;
    let plain = sealed.cell(plain, &sources)?;
    let encrypted = sealed.cell(encrypted, &sources)?;
    assert!(
        matches!(
            capture.seal(&sources, None, &[]),
            Err(CatalogError::Accounting(
                ExportAccountingError::HealthCaptureClosed
            ))
        ),
        "the report's text store seals exactly once",
    );
    let metadata = sealed.into_metadata();
    storage(&metadata, &plain, &encrypted)?;
    occurrences(&plain, &encrypted);
    inventory(&sources)
}

fn storage(
    metadata: &HealthMetadata,
    plain: &HealthTextCell,
    encrypted: &HealthTextCell,
) -> TestResult {
    assert_eq!(
        plain.cell().value(),
        encrypted.cell().value(),
        "equal source values share storage only"
    );
    assert_eq!(
        metadata.texts().len(),
        1,
        "the shared exact bytes are retained once"
    );
    let atom = metadata
        .text(*plain.cell().value().ok_or("present plain semantic cell")?)
        .ok_or("actual shared atom")?;
    assert_eq!(
        atom.origin().contains_encrypted_source(),
        Some(true),
        "aggregate storage retains the known encrypted observation",
    );
    Ok(())
}

fn occurrences(plain: &HealthTextCell, encrypted: &HealthTextCell) {
    let plain_view = plain.view();
    let encrypted_view = encrypted.view();
    assert_eq!(
        plain_view.value(),
        Some("same"),
        "the real consumer resolves the plain value"
    );
    assert_eq!(
        plain_view.origin().contains_encrypted_source(),
        Some(false),
        "the plain semantic datum keeps only its own original input",
    );
    assert_eq!(
        encrypted_view.origin().contains_encrypted_source(),
        Some(true),
        "the encrypted semantic datum remains encrypted",
    );
    let visible: Vec<_> = [plain, encrypted]
        .into_iter()
        .map(HealthTextCell::view)
        .filter(|view| view.origin().contains_encrypted_source() == Some(false))
        .filter_map(|view| view.value())
        .collect();
    assert_eq!(
        visible,
        ["same"],
        "semantic filtering keeps the plain occurrence despite the encrypted aggregate atom",
    );
}

fn inventory(sources: &SourceUses) -> TestResult {
    let inventory = sources.freeze()?;
    let payloads = inventory
        .uses()
        .iter()
        .filter(|used| {
            used.role() == SourceUseRole::Payload
                && matches!(
                    used.location(),
                    SourceLocation::Subresource { namespace, field, .. }
                        if namespace == "syntheticText" && field == "$value"
                )
        })
        .count();
    assert_eq!(
        payloads, 2,
        "the one registry retains both exact original payload contributions"
    );
    Ok(())
}

fn original(archive: &Archive, suffix: &str) -> TestResult<String> {
    use std::io::Read as _;

    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path().ends_with(suffix))
        .ok_or("actual original text Entry")?;
    assert!(
        archive.observed_entry(entry).is_some(),
        "the original Entry belongs to its owner"
    );
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    assert_eq!(bytes, b"same", "the actual original payload read succeeds");
    Ok(String::from_utf8(bytes)?)
}

fn fact<'a>(
    sources: &SourceUses,
    archive: &Archive,
    suffix: &str,
    text: &'a str,
) -> TestResult<OriginalDatum<&'a str>> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path().ends_with(suffix))
        .ok_or("actual selected original")?;
    let origin = archive
        .entry_origin(entry)
        .ok_or("genuine original Entry snapshot")?;
    let scope = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let input = scope.at(
        SourceLocation::Subresource {
            namespace: "syntheticText".to_owned(),
            indices: vec![u64::try_from(entry.index())?],
            field: "$value".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    Ok(OriginalDatum::observed(
        SourceValue::Present(text),
        vec![input],
        true,
    ))
}
