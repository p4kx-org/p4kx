// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
    write_build_manifest,
};
use serde_json::{Value, json};

use super::resource_declarations::Scope;
use super::resource_metadata;
use super::resource_metadata_output::Destination;

const DRIVE: &str = "SCItemQuantumDriveParams";
const PROFILE: &str = "SQuantumDriveParams";
const EFFECT: &str = "SQuantumDriveEffectBaseParams";
const LEGACY: &str = "SQuantumDriveEffectParams_LEGACY";
const TEMPLATE: &str = "SQuantumDriveEffectTemplateRef";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn quantum_effect_declarations_only() -> super::Result<()> {
    resource_metadata::run(Scope::QuantumDrives)
}

#[test]
fn quantum_metadata_preserves_manifest_and_uninstantiated_declarations() -> super::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = fs::canonicalize(directory.path())?;
    let channel = root.join("TECH-PREVIEW");
    let path = fixture_file(&channel, TestEntry::stored("Data/Game2.dcb", database()))?;
    manifest(&channel)?;
    let output = root.join("quantum-declarations.json");
    resource_metadata::read_and_publish(
        &Archive::open(path)?,
        &Destination::new(&root, &output)?,
        Scope::QuantumDrives,
    )?;
    let bytes = fs::read(output)?;
    let text = std::str::from_utf8(&bytes)?;
    assert!(
        !text.contains("SENTINEL"),
        "Record identities and data stay private"
    );
    let document: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        document.get("sourceBuild"),
        Some(&json!({
            "channel": "TECH-PREVIEW",
            "branch": "fixture-branch",
            "version": "1.2.3",
            "changeNumber": "456",
            "buildId": "fixture-build",
        })),
        "the five exact manifest and Channel fields travel with the declarations",
    );
    verify_declarations(&document)
}

fn verify_declarations(document: &Value) -> super::Result<()> {
    assert_eq!(
        document.pointer("/declarations/missingRoots"),
        Some(&json!([TEMPLATE])),
        "missing owners are distinct from declared empty owners",
    );
    assert!(
        field_rows(document, EFFECT)?.is_empty(),
        "a declared base type may own no fields",
    );
    assert_eq!(
        field_rows(document, LEGACY)?,
        &[json!({
            "name": "spoolEffectTag",
            "dataType": "String",
            "array": false,
            "targetType": null,
        })],
        "a declared field is captured even with no concrete owner instance",
    );
    assert_eq!(
        field_rows(document, PROFILE)?,
        &[json!({
            "name": "effectTags",
            "dataType": "StrongPointer",
            "array": false,
            "targetType": EFFECT,
        })],
        "an owning pointer retains its declared target",
    );
    assert!(
        field_rows(document, DRIVE)?
            .iter()
            .all(|field| field.get("name").and_then(Value::as_str) != Some("quantumBoostParams")),
        "the survey does not fabricate an absent declaration",
    );
    Ok(())
}

#[test]
fn quantum_metadata_refuses_an_unidentified_or_corrupt_source() -> super::Result<()> {
    let directory = tempfile::tempdir()?;
    let root = fs::canonicalize(directory.path())?;
    let valid = root.join("TECH-PREVIEW");
    let path = fixture_file(&valid, TestEntry::stored("Data/Game2.dcb", database()))?;
    let output = root.join("quantum-declarations.json");
    let destination = Destination::new(&root, &output)?;
    let missing = resource_metadata::read_and_publish(
        &Archive::open(path)?,
        &destination,
        Scope::QuantumDrives,
    )
    .err()
    .ok_or("quantum metadata was published without a Build")?;
    assert!(
        missing.to_string().contains("complete Build"),
        "the witness requires exact Build authority",
    );
    assert!(
        !output.exists(),
        "the unidentified source publishes no artifact"
    );

    let corrupt = root.join("PTU");
    let path = fixture_file(
        &corrupt,
        TestEntry::stored("Data/Game2.dcb", database()).with_crc32c(1),
    )?;
    manifest(&corrupt)?;
    let mismatch = resource_metadata::read_and_publish(
        &Archive::open(path)?,
        &destination,
        Scope::QuantumDrives,
    )
    .err()
    .ok_or("quantum metadata was published after a checksum mismatch")?;
    assert!(
        mismatch.to_string().contains("crc32c"),
        "the mismatched DCB checksum is diagnosed",
    );
    assert!(!output.exists(), "the corrupt source publishes no artifact");
    Ok(())
}

fn field_rows<'a>(document: &'a Value, owner: &str) -> super::Result<&'a [Value]> {
    document
        .pointer("/declarations/types")
        .and_then(Value::as_array)
        .and_then(|types| {
            types
                .iter()
                .find(|row| row.get("name").and_then(Value::as_str) == Some(owner))
        })
        .and_then(|row| row.get("fields").and_then(Value::as_array))
        .map(Vec::as_slice)
        .ok_or_else(|| format!("no declaration for fixture owner {owner}").into())
}

fn fixture_file(channel: &Path, entry: TestEntry) -> super::Result<PathBuf> {
    fs::create_dir(channel)?;
    let path = channel.join("Data.p4k");
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    Ok(path)
}

fn manifest(channel: &Path) -> std::io::Result<()> {
    write_build_manifest(channel, "fixture-branch", "1.2.3", "456", "fixture-build")
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new(DRIVE)
                .field("description", TestType::String)
                .field("params", TestType::Class(PROFILE.to_owned())),
        )
        .structure(
            TestStruct::new(PROFILE)
                .field("effectTags", TestType::StrongPointer(EFFECT.to_owned())),
        )
        .structure(TestStruct::new(EFFECT))
        .structure(
            TestStruct::new(LEGACY)
                .parent(EFFECT)
                .field("spoolEffectTag", TestType::String),
        )
        .record(
            TestRecord::new(0x1234, DRIVE, "RECORD_SENTINEL", "PATH_SENTINEL.xml")
                .value("description", TestValue::text("VALUE_SENTINEL")),
        )
        .build()
}
