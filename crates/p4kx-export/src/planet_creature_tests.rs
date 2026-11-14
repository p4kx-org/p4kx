// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::{CatalogOptions, CatalogPublicationMode, build_catalog};

#[path = "planet_creature_witness.rs"]
mod witness;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ENTITY_GUID: &str = "00000000-0000-0000-0000-000000000010";
const WRONG_GUID: &str = "00000000-0000-0000-0000-000000000020";
const MISSING_GUID: &str = "00000000-0000-0000-0000-000000000030";
const FAUNA_GUID: &str = "00000000-0000-0000-0000-000000000040";

fn resource(stem: &str, data: impl Into<Vec<u8>>) -> TestEntry {
    TestEntry::stored(format!("Data/Libs/Planets/creatures/{stem}.creature"), data)
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("Data.p4k");
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("OtherCreatureKind"))
        .record(TestRecord::new(
            0x10,
            "EntityClassDefinition",
            "Fixture",
            "actors/fixture.xml",
        ))
        .record(TestRecord::new(
            0x20,
            "OtherCreatureKind",
            "Other",
            "actors/other.xml",
        ))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", forge))
        .entry(resource("source", format!(r#"{{"guid":"{FAUNA_GUID}","entityClass":"{ENTITY_GUID}","needVerticalOffset":false,"slopeTolerance":-1.23456789012345678900e+2}}"#)).encrypted())
        .entry(resource("wrong", format!(r#"{{"guid":"{FAUNA_GUID}","entityClass":"{WRONG_GUID}","needVerticalOffset":null}}"#)))
        .entry(resource("missing", format!(r#"{{"entityClass":"{MISSING_GUID}","slopeTolerance":"bad"}}"#)))
        .entry(resource("broken", "{"))
        .entry(TestEntry::stored("Data/Libs/Planets/creaturesAdjacent/ignored.creature", "{}"))
        .write_to(&path)?;
    Ok((
        temp,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn catalog(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = filter;
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let output = build_catalog(archive, &options)?;
    Ok(serde_json::from_slice(output.json())?)
}

fn rows(catalog: &Value) -> TestResult<&[Value]> {
    catalog
        .pointer("/definitions/creatures/rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "serialized fauna definitions".into())
}

#[test]
fn fauna_preserves_typed_wire_and_resolves_exact_entity_guid() -> TestResult {
    let (_temp, archive) = fixture()?;
    let output = catalog(&archive, None)?;
    let rows = rows(&output)?;
    assert_eq!(rows.len(), 3, "only readable enrolled sources are rows");
    let source = rows.first().ok_or("first physical source")?;
    assert_eq!(
        source.pointer("/guid/value"),
        Some(&json!(FAUNA_GUID)),
        "original fauna identity"
    );
    assert_eq!(
        source.pointer("/entityClass/value"),
        Some(&json!(ENTITY_GUID)),
        "authored exact Entity GUID"
    );
    assert_eq!(
        source.pointer("/entity/state/value"),
        Some(&json!("resolved")),
        "record type and physical header"
    );
    assert_eq!(
        source.pointer("/entity/candidates/0/sourceType/value"),
        Some(&json!("EntityClassDefinition")),
        "verified physical target type",
    );
    assert_eq!(
        source.pointer("/slopeTolerance/value"),
        Some(&json!("-1.23456789012345678900e+2")),
        "keeps lexical numeric precision",
    );
    assert_eq!(
        source.pointer("/needVerticalOffset/value"),
        Some(&json!(false)),
        "explicit false"
    );
    assert_eq!(
        source.pointer("/slopeTolerance/containsEncryptedSource"),
        Some(&json!(true)),
        "exact encrypted source cell",
    );
    assert_eq!(
        source.pointer("/issues/0/code"),
        Some(&json!("ambiguousIdentity")),
        "duplicate definition GUIDs stay separate",
    );
    Ok(())
}

#[test]
fn fauna_retains_wrong_family_missing_filtered_and_invalid_states() -> TestResult {
    let (_temp, archive) = fixture()?;
    let output = catalog(&archive, None)?;
    let unfiltered_rows = rows(&output)?;
    let wrong = unfiltered_rows.get(1).ok_or("wrong-family row")?;
    let missing = unfiltered_rows.get(2).ok_or("missing target row")?;
    assert_eq!(
        wrong.pointer("/entity/state/value"),
        Some(&json!("wrongFamily")),
        "actual colliding Record type"
    );
    assert_eq!(
        wrong.pointer("/needVerticalOffset/state"),
        Some(&json!("null")),
        "explicit null"
    );
    assert_eq!(
        missing.pointer("/guid/state"),
        Some(&json!("absent")),
        "absent identity"
    );
    assert_eq!(
        missing.pointer("/entity/state/value"),
        Some(&json!("missing")),
        "dangling link"
    );
    assert_eq!(
        missing.pointer("/slopeTolerance/state"),
        Some(&json!("invalid")),
        "invalid numeric storage"
    );
    let only_source = Filter::new("source.creature");
    let filtered = catalog(&archive, Some(&only_source))?;
    let filtered_rows = rows(&filtered)?;
    assert_eq!(
        filtered_rows.len(),
        1,
        "filter applies to fauna Entry paths"
    );
    let first = filtered_rows.first().ok_or("selected definition")?;
    assert_eq!(
        first.pointer("/entity/state/value"),
        Some(&json!("filtered")),
        "target remains independently inspected"
    );
    assert_eq!(
        filtered.pointer("/completeness/creatures/status"),
        Some(&json!("partial")),
        "visible Filter scope"
    );
    Ok(())
}
