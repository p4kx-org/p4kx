// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{Index, MediaKind, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn standalone_material_cli_export_keeps_original_bytes_and_uses_the_builtin_schema() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let source = br#"<Material RootOnly="keep"><MatLayers><Layer Path="missing" Submtl="007"/></MatLayers></Material>"#;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/unused.mtl", source))
        .write_to(&path)?;
    let output = folder.path().join("material");
    p4kx_testkit::p4kx()?
        .args(["media", "export"])
        .arg(&path)
        .arg("--out")
        .arg(&output)
        .args(["--kind", "material"])
        .assert()
        .code(0);
    let index: Index = serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
    assert_eq!(
        index
            .kinds
            .iter()
            .map(|kind| kind.provider.kind)
            .collect::<Vec<_>>(),
        [MediaKind::Material],
        "explicit selection runs only the available material provider"
    );
    let lines = fs::read_to_string(output.join(&index.artifact_base).join("material.jsonl"))?;
    let mut rows = lines.lines();
    let record: MediaRecord = serde_json::from_str(rows.next().ok_or("material row")?)?;
    assert!(
        rows.next().is_none(),
        "an unused standalone library needs no model or geometry"
    );
    let raw = record.source.raw.as_ref().ok_or("exact original source")?;
    assert_eq!(
        fs::read(output.join(&index.artifact_base).join(&raw.path))?,
        source,
        "CLI retains the original decoded XML bytes independently"
    );
    validate_material_schema(record)
}

fn validate_material_schema(record: MediaRecord) -> TestResult {
    let value = serde_json::to_value(record)?;
    assert_eq!(
        value
            .pointer("/data/payload/references/0/submaterial")
            .and_then(Value::as_str),
        Some("007"),
        "the original Submtl stays a name"
    );
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    assert!(
        validator.is_valid(&value),
        "CLI row validates against its common builtin schema"
    );
    let mut wrong = value;
    *wrong
        .pointer_mut("/data/record_type")
        .ok_or("record type")? = Value::String("failure".into());
    assert!(
        !validator.is_valid(&wrong),
        "the material-library namespace rejects the failure payload"
    );
    Ok(())
}
