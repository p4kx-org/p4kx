// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Actual input scopes and immediate per-value origin on shared serialization carriers.

use std::error::Error;
use std::fs;

use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeOrigin, InputScope, SourceInput, SourceLocation,
    SourceUseRole, SourceUses, SourceValue, SourceValueState, SourcedValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;

#[path = "field_origin/schema.rs"]
mod schema;

type TestResult = Result<(), Box<dyn Error>>;

fn open(builder: &ArchiveBuilder) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    builder.write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn field(name: &str) -> SourceLocation {
    SourceLocation::DataForge {
        record_index: 0,
        field: name.to_owned(),
    }
}

#[test]
fn one_serialized_row_exposes_plain_encrypted_and_unknown_field_origins() -> TestResult {
    let builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("same.bin", b"a"))
        .entry(TestEntry::stored("same.bin", b"a").encrypted())
        .entry(TestEntry::stored("unknown.bin", b"a").with_encryption_field(7));
    let (_directory, archive) = open(&builder)?;
    let sources = SourceUses::new();
    let mut values = Vec::new();
    for (index, entry) in archive.entries().iter().enumerate() {
        let origin = archive.entry_origin(entry).ok_or("origin missing")?;
        let scope = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
        let used = scope.at(field(&format!("value{index}")), SourceUseRole::Payload)?;
        values.push(sources.value(SourceValue::Present(0_u32), &[used], true));
    }
    let row = serde_json::to_value(&values)?;
    assert_eq!(row[0]["containsEncryptedSource"], false);
    assert_eq!(row[1]["containsEncryptedSource"], true);
    assert!(
        row[2]["containsEncryptedSource"].is_null(),
        "unsupported declaration stays unknown"
    );
    for value in row.as_array().ok_or("value array missing")? {
        assert_eq!(value["value"], 0);
        assert_eq!(value["state"], "present");
        assert_eq!(value["sourceSetComplete"], true);
        assert_eq!(
            value["sourceUsesRefs"]
                .as_array()
                .ok_or("use references missing")?
                .len(),
            1
        );
    }
    let inventory = serde_json::to_value(sources.freeze()?)?;
    let physical = inventory["sources"].as_array().ok_or("sources missing")?;
    assert_eq!(
        physical.len(),
        3,
        "equal payloads and paths retain distinct Entry indices"
    );
    assert_ne!(physical[0]["id"], physical[1]["id"]);
    Ok(())
}

#[test]
fn leaf_payload_encryption_does_not_taint_its_directory_metadata() -> TestResult {
    let (_directory, archive) = open(
        &ArchiveBuilder::new().entry(TestEntry::stored("encrypted.bin", b"payload").encrypted()),
    )?;
    let origin = archive
        .entry_origin(archive.entries().first().ok_or("Entry missing")?)
        .ok_or("origin missing")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    let payload = scope.at(field("payload"), SourceUseRole::Payload)?;
    let metadata = scope.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let payload = sources.value(SourceValue::Present("payload"), &[payload], true);
    let path = sources.value(SourceValue::Present("encrypted.bin"), &[metadata], true);
    assert_eq!(payload.origin().contains_encrypted_source(), Some(true));
    assert_eq!(path.origin().contains_encrypted_source(), Some(false));
    let inventory = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(inventory["sources"][0]["storedEncryption"], "encrypted");
    assert_eq!(
        inventory["sources"][0]["path"]["containsEncryptedSource"],
        false
    );
    assert_eq!(
        inventory["sources"][0]["declarations"]["value"][0]["value"]["rawWord"]["containsEncryptedSource"],
        false
    );
    Ok(())
}

#[test]
fn encrypted_ancestors_taint_the_inner_directory_as_well_as_payload() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.bin", b"payload"))
        .build();
    let (_directory, root) =
        open(&ArchiveBuilder::new().entry(TestEntry::stored("room.socpak", inner).encrypted()))?;
    let inner = root.open_container(root.entries().first().ok_or("container missing")?)?;
    let origin = inner
        .entry_origin(inner.entries().first().ok_or("inner Entry missing")?)
        .ok_or("inner origin missing")?;
    let sources = SourceUses::new();
    let scope = sources.observe(
        SourceInput::Archive(&origin),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let used = scope.at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let value = sources.value(SourceValue::Present("plain.bin"), &[used], true);
    assert_eq!(value.origin().contains_encrypted_source(), Some(true));
    let inventory = serde_json::to_value(sources.freeze()?)?;
    assert_eq!(
        inventory["sources"]
            .as_array()
            .ok_or("sources missing")?
            .len(),
        2
    );
    let inner = inventory["sources"]
        .as_array()
        .ok_or("sources missing")?
        .iter()
        .find(|row| row["path"]["value"] == "plain.bin")
        .ok_or("inner source missing")?;
    assert_eq!(inner["storedEncryption"], "unencrypted");
    assert_eq!(inner["path"]["containsEncryptedSource"], true);
    Ok(())
}

#[test]
fn attempts_and_untouched_targets_do_not_enter_actual_field_contributions() -> TestResult {
    let (_directory, archive) = open(
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("plain.bin", b"a"))
            .entry(TestEntry::stored("secret.bin", b"a").encrypted()),
    )?;
    let plain = archive
        .entry_origin(archive.entries().first().ok_or("plain Entry missing")?)
        .ok_or("plain origin missing")?;
    let secret = archive
        .entry_origin(archive.entries().get(1).ok_or("secret Entry missing")?)
        .ok_or("secret origin missing")?;
    let sources = SourceUses::new();
    let plain = sources.observe(SourceInput::Archive(&plain), InputScope::Payload)?;
    let secret = sources.observe(SourceInput::Archive(&secret), InputScope::Payload)?;
    let uses = [
        plain.at(field("value"), SourceUseRole::Payload)?,
        secret.at(field("unread"), SourceUseRole::Attempted)?,
        secret.at(field("target"), SourceUseRole::Reference)?,
    ];
    let value = sources.value(SourceValue::Present(false), &uses, true);
    assert_eq!(value.origin().contains_encrypted_source(), Some(false));
    assert_eq!(value.origin().source_uses_refs().len(), 1);
    let inventory = sources.freeze()?;
    assert!(
        inventory
            .uses()
            .iter()
            .any(|value| value.role() == SourceUseRole::Attempted),
        "attempt remains separately visible"
    );
    assert!(
        inventory
            .uses()
            .iter()
            .any(|value| value.role() == SourceUseRole::Reference),
        "untouched reference remains separately visible"
    );
    Ok(())
}

#[test]
fn complete_empty_unknown_and_foreign_sets_remain_distinct() -> TestResult {
    let sources = SourceUses::new();
    let empty = sources.value(SourceValue::Present(0), &[], true);
    let incomplete = sources.value(SourceValue::<u32>::Unreadable, &[], false);
    assert_eq!(empty.origin().contains_encrypted_source(), Some(false));
    assert_eq!(incomplete.origin().contains_encrypted_source(), None);
    assert_eq!(incomplete.state(), SourceValueState::Unreadable);
    let other = SourceUses::new();
    let unknown = other
        .observe(SourceInput::Unknown, InputScope::Payload)?
        .at(field("foreign"), SourceUseRole::Payload)?;
    let value = sources.value(SourceValue::Present(1), &[unknown], true);
    assert_eq!(value.origin().contains_encrypted_source(), None);
    assert!(
        !value.origin().source_set_complete(),
        "foreign inventory cannot satisfy local proof"
    );
    assert!(
        value.origin().source_uses_refs().is_empty(),
        "foreign IDs never bind by spelling"
    );
    Ok(())
}

#[test]
fn file_inputs_keep_unknown_origin_and_never_publish_machine_paths() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("same.dcb");
    fs::write(&path, b"synthetic bytes")?;
    let read = DataForgeEntry::read_file(&path)?;
    let other = DataForgeEntry::read_file(&path)?;
    let (DataForgeOrigin::File(first), DataForgeOrigin::File(second)) =
        (read.origin(), other.origin())
    else {
        return Err("File origins missing".into());
    };
    let sources = SourceUses::new();
    let first = sources.observe(SourceInput::File(first), InputScope::Payload)?;
    let second = sources.observe(SourceInput::File(second), InputScope::Payload)?;
    assert_ne!(first.source_ref(), second.source_ref());
    let used = first.at(field("value"), SourceUseRole::Payload)?;
    let value = sources.value(SourceValue::Present(1), &[used], true);
    assert_eq!(value.origin().contains_encrypted_source(), None);
    let json = serde_json::to_string(&sources.freeze()?)?;
    assert!(
        !json.contains(directory.path().to_string_lossy().as_ref()),
        "host directory is private"
    );
    assert!(
        json.contains("same.dcb"),
        "sanitized source name remains available"
    );
    Ok(())
}

#[test]
fn field_schema_requires_nullable_origin_and_value_keys() -> TestResult {
    let schema = serde_json::to_value(schemars::schema_for!(SourcedValue<u32>))?;
    let required = schema["required"]
        .as_array()
        .ok_or("required fields missing")?;
    for key in [
        "state",
        "value",
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        assert!(
            required.contains(&json!(key)),
            "required key missing: {key}"
        );
    }
    let sources = SourceUses::new();
    let value = sources
        .value(SourceValue::<u32>::Null, &[], true)
        .map(|value| value + 1);
    let encoded = serde_json::to_value(value)?;
    assert_eq!(encoded["state"], "null");
    assert!(encoded["value"].is_null(), "map retains explicit null");
    Ok(())
}
