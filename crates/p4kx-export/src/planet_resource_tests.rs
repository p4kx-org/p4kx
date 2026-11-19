// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, CancelToken, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog};

#[path = "planet_resource_query_tests.rs"]
mod query;
#[path = "planet_resource_sdk_witness.rs"]
mod sdk_witness;
#[path = "planet_resource_witness.rs"]
mod witness;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const SAMPLE: &str = "Data/Libs/Planets/resources/";
const VALID: &str = "AAAAAAAA-0000-0000-0000-000000000001";
const ZERO: &str = "00000000-0000-0000-0000-000000000000";
const PALETTE: &str = "libs/foundry/records/tintpalettes/palette-fixture";
const NON_PALETTE: &str = "libs/foundry/records/tintpalettes/incorrect-fixture";
const FILE_PALETTE: &str = "Fixtures/Palette.data";

fn fixture(entries: Vec<TestEntry>, key_given: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("TintPaletteTree"))
        .structure(TestStruct::new("OtherPaletteFixture"))
        .record(TestRecord::new(
            7,
            "TintPaletteTree",
            "PaletteFixture",
            format!("{PALETTE}.xml"),
        ))
        .record(TestRecord::new(
            8,
            "OtherPaletteFixture",
            "OtherFixture",
            format!("{NON_PALETTE}.xml"),
        ))
        .build();
    let mut archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", forge))
        .entry(TestEntry::stored(
            format!("Data/{FILE_PALETTE}"),
            "archive metadata target",
        ));
    for entry in entries {
        archive = archive.entry(entry);
    }
    archive.write_to(&path)?;
    let key = key_given.then(p4kx_testkit::test_key);
    Ok((dir, Archive::open_with_key(path, key)?))
}

fn resource(stem: &str, document: impl Into<Vec<u8>>) -> TestEntry {
    TestEntry::stored(format!("{SAMPLE}{stem}.resource"), document)
}

fn json_catalog(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let catalog = build_catalog(archive, &options)?;
    Ok(serde_json::from_slice(catalog.json())?)
}

fn rows(catalog: &Value) -> TestResult<&[Value]> {
    catalog
        .pointer("/definitions/planetResources/rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "planet resource rows".into())
}

fn row<'a>(rows: &'a [Value], suffix: &str) -> TestResult<&'a Value> {
    rows.iter()
        .find(|row| {
            row.pointer("/identity/sourcePath/value")
                .and_then(Value::as_str)
                .is_some_and(|path| path.ends_with(suffix))
        })
        .ok_or_else(|| format!("missing expected synthetic row {suffix}").into())
}

#[test]
fn planet_resources_keep_typed_atoms_and_exact_occurrence_and_target_provenance() -> TestResult {
    let (_folder, archive) = fixture(vec![
        resource("record", format!(r#"{{"guid":"{{{VALID}}}","name":"fixture","bMinable":false,"tintPalettePath":"{PALETTE}"}}"#)).encrypted(),
        resource("archive", format!(r#"{{"guid":"{ZERO}","name":"","bMinable":true,"tintPalettePath":"{FILE_PALETTE}"}}"#)),
        resource("wrong", format!(r#"{{"guid":"invalid raw","bMinable":null,"tintPalettePath":"{NON_PALETTE}"}}"#)),
        resource("duplicated-identity", format!(r#"{{"guid":"{VALID}","tintPalettePath":""}}"#)),
    ], true)?;
    let catalog = json_catalog(&archive, None)?;
    let rows = rows(&catalog)?;
    let record = row(rows, "record.resource")?;
    let archive_entry = row(rows, "archive.resource")?;
    let wrong = row(rows, "wrong.resource")?;
    let duplicate = row(rows, "duplicated-identity.resource")?;
    assert_dataforge_record(record);
    assert_archive_and_duplicate(archive_entry, duplicate);
    assert_invalid_record(wrong);
    Ok(())
}

fn assert_dataforge_record(record: &Value) {
    assert_eq!(
        record.pointer("/guid/value"),
        Some(&json!(VALID.to_ascii_lowercase())),
        "canonical GUID"
    );
    assert_eq!(
        record.pointer("/guidRaw/value"),
        Some(&json!(format!("{{{VALID}}}"))),
        "original GUID"
    );
    assert_eq!(
        record.pointer("/name/containsEncryptedSource"),
        Some(&json!(true)),
        "encrypted owner"
    );
    assert_eq!(
        record.pointer("/minable/value"),
        Some(&json!(false)),
        "explicit false"
    );
    assert_eq!(
        record.pointer("/tintPalette/targetKind"),
        Some(&json!("dataForge")),
        "declared palette reference"
    );
    assert_eq!(
        record.pointer("/tintPalette/state/value"),
        Some(&json!("resolved")),
        "exact original target"
    );
    assert_eq!(
        record.pointer("/tintPalette/candidates/0/sourceType/value"),
        Some(&json!("TintPaletteTree")),
        "target type"
    );
    assert_eq!(
        record.pointer("/issues/0/code"),
        Some(&json!("ambiguousIdentity")),
        "physically duplicated GUID retained"
    );
}

fn assert_archive_and_duplicate(archive_entry: &Value, duplicate: &Value) {
    assert_eq!(
        duplicate.pointer("/tintPalette/state/value"),
        Some(&json!("empty")),
        "empty path"
    );
    assert_eq!(
        archive_entry.pointer("/guid/value"),
        Some(&json!(ZERO)),
        "zero GUID kept"
    );
    assert_eq!(
        archive_entry.pointer("/name/value"),
        Some(&json!("")),
        "empty label"
    );
    assert_eq!(
        archive_entry.pointer("/tintPalette/resolution/state/value"),
        Some(&json!("resolved")),
        "exact Archive target"
    );
}

fn assert_invalid_record(wrong: &Value) {
    assert_eq!(
        wrong.pointer("/guid/state"),
        Some(&json!("invalid")),
        "bad identifier"
    );
    assert_eq!(
        wrong.pointer("/guidRaw/value"),
        Some(&json!("invalid raw")),
        "bad identifier as authored"
    );
    assert_eq!(
        wrong.pointer("/name/state"),
        Some(&json!("absent")),
        "absent name"
    );
    assert_eq!(
        wrong.pointer("/minable/state"),
        Some(&json!("null")),
        "explicit null"
    );
    assert_eq!(
        wrong.pointer("/tintPalette/state/value"),
        Some(&json!("wrongFamily")),
        "unrelated Record shape"
    );
}

#[test]
fn planet_resources_keep_readable_siblings_and_filter_scope() -> TestResult {
    let (_folder, archive) = fixture(
        vec![
            resource(
                "selected",
                r#"{"bMinable":true,"tintPalettePath":"../forbidden"}"#,
            ),
            resource("broken", "{"),
            resource(
                "missing",
                r#"{"guid":"00000000-0000-0000-0000-000000000000","tintPalettePath":"Fixtures/Missing.data"}"#,
            ),
            resource("null", r#"{"tintPalettePath":null,"bMinable":"true"}"#),
            resource("duplicate-member", r#"{"guid":"one","g\u0075id":"two"}"#),
            TestEntry::stored("Data/Libs/Planets/resourcesAdjacent/out.resource", "{}"),
        ],
        true,
    )?;
    assert!(
        matches!(
            build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH)),
            Err(CatalogError::IncompleteExport(_))
        ),
        "invalid JSON blocks normal publication"
    );
    assert_diagnostic_resource_rows(&json_catalog(&archive, None)?)?;
    let filter = Filter::new("selected.resource");
    let first = json_catalog(&archive, Some(&filter))?;
    let second = json_catalog(&archive, Some(&filter))?;
    assert_eq!(first, second, "repeated filtered exports are deterministic");
    assert_eq!(
        rows(&first)?.len(),
        1,
        "only selected physical entries are exported"
    );
    assert_eq!(
        first.pointer("/completeness/planetResources/status"),
        Some(&json!("partial")),
        "catalog scope is filter-aware"
    );
    Ok(())
}

fn assert_diagnostic_resource_rows(catalog: &Value) -> TestResult {
    let resource_rows = rows(catalog)?;
    assert_eq!(
        resource_rows.len(),
        4,
        "diagnostic mode keeps readable resources"
    );
    let selected = row(resource_rows, "selected.resource")?;
    assert_eq!(
        selected.pointer("/tintPalette/state/value"),
        Some(&json!("invalid")),
        "unsafe path preserved and diagnosed"
    );
    assert_eq!(
        selected.pointer("/tintPalettePath/value"),
        Some(&json!("../forbidden")),
        "authored path"
    );
    assert_eq!(
        row(resource_rows, "missing.resource")?.pointer("/tintPalette/resolution/state/value"),
        Some(&json!("missing")),
        "missing metadata target"
    );
    assert_eq!(
        row(resource_rows, "null.resource")?.pointer("/tintPalette/state/value"),
        Some(&json!("null")),
        "explicit null"
    );
    assert_eq!(
        row(resource_rows, "duplicate-member.resource")?.pointer("/guid/state"),
        Some(&json!("invalid")),
        "escaped duplicate key rejected as unique value"
    );
    Ok(())
}

#[test]
fn planet_resources_keep_deep_valid_entries_and_diagnose_bad_entry_integrity() -> TestResult {
    let nesting = 257;
    let original = format!(
        r#"{{"bMinable":true,"additional":{}0{}}}"#,
        "[".repeat(nesting),
        "]".repeat(nesting)
    );
    let (_folder, archive) = fixture(
        vec![
            resource("deep", original),
            resource("bad-crc", "{}").with_crc32c(0),
            resource("bad-length", "{}").with_uncompressed_size(3),
        ],
        true,
    )?;
    let catalog = json_catalog(&archive, None)?;
    assert_eq!(
        rows(&catalog)?.len(),
        1,
        "valid deep JSON survives corrupt sibling Entries"
    );
    assert_eq!(
        row(rows(&catalog)?, "deep.resource")?.pointer("/minable/value"),
        Some(&json!(true)),
        "typed fields survive independent deep data",
    );
    let failures = catalog
        .pointer("/exportHealth/failedEntries")
        .and_then(Value::as_array)
        .ok_or("catalog Entry read failures")?;
    assert_eq!(
        failures.len(),
        2,
        "declared length and checksum failures remain distinct physical entries"
    );
    Ok(())
}

#[test]
fn planet_resources_support_absent_families_and_cancellation() -> TestResult {
    let (_folder, archive) = fixture(Vec::new(), false)?;
    let catalog = json_catalog(&archive, None)?;
    assert_eq!(
        catalog.pointer("/definitions/planetResources/availability"),
        Some(&json!("absent")),
        "definition family proved absent"
    );
    assert_eq!(rows(&catalog)?.len(), 0, "empty Resource dataset");
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.cancel = Some(&cancel);
    assert!(
        matches!(
            build_catalog(&archive, &options),
            Err(CatalogError::Cancelled)
        ),
        "accepted cancellation reaches the shared build"
    );
    Ok(())
}
