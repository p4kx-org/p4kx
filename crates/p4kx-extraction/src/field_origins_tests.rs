// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::origin_reader_tests::reader_limits;
use crate::{FieldOriginSeed, FieldOriginsSeed, FieldPathSeed};
use serde::de::DeserializeSeed as _;
use serde_json::{Value, json};

use crate::{
    FieldOrigin, FieldOrigins, FieldPath, InputScope, SourceLocation, SourceScope, SourceUseRole,
    SourceUses, SourceValue,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn field_paths_preserve_empty_keys_slashes_tildes_and_original_positions() -> TestResult {
    let mut path = FieldPath::root();
    path.push_field("data")?;
    path.push_field("a/b~c")?;
    path.push_field("")?;
    path.push_index(12)?;
    assert_eq!(
        path.as_str(),
        "/data/a~1b~0c//12",
        "each authored key has its exact escaped segment"
    );
    path.pop();
    assert_eq!(
        path.as_str(),
        "/data/a~1b~0c/",
        "popping an index preserves the empty key"
    );
    assert!(
        FieldPath::parse("data/value").is_err(),
        "relative field addresses are rejected"
    );
    assert!(
        FieldPath::parse("/data/~2").is_err(),
        "invalid escapes are rejected"
    );
    let encoded = serde_json::to_string(&path)?;
    assert_eq!(
        FieldPathSeed::new(reader_limits().path_bytes)
            .deserialize(&mut serde_json::Deserializer::from_str(&encoded))?,
        path,
        "the typed pointer round-trips"
    );
    Ok(())
}

#[test]
fn a_present_field_origin_requires_all_marker_keys_and_consistent_empty_sets() -> TestResult {
    let valid =
        json!({"containsEncryptedSource": false, "sourceUsesRefs": [], "sourceSetComplete": true});
    let origin: FieldOrigin = FieldOriginSeed::new(reader_limits()).deserialize(valid.clone())?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "a complete empty physical set is explicit"
    );
    assert!(
        origin.source_set_complete(),
        "the complete empty set remains complete"
    );
    for key in [
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        let mut missing = valid.clone();
        missing.as_object_mut().ok_or("origin object")?.remove(key);
        assert!(
            FieldOriginSeed::new(reader_limits())
                .deserialize(missing)
                .is_err(),
            "a present origin cannot omit {key}"
        );
    }
    for (marker, complete) in [
        (json!(true), true),
        (json!(true), false),
        (json!(false), false),
        (json!(null), true),
    ] {
        let invalid = json!({"containsEncryptedSource": marker, "sourceUsesRefs": [], "sourceSetComplete": complete});
        assert!(
            FieldOriginSeed::new(reader_limits())
                .deserialize(invalid)
                .is_err(),
            "empty-set marker correlation is enforced"
        );
    }
    let unknown: FieldOrigin = FieldOriginSeed::new(reader_limits())
        .deserialize(json!({"containsEncryptedSource": null,
        "sourceUsesRefs": [], "sourceSetComplete": false}))?;
    assert_eq!(
        unknown.contains_encrypted_source(),
        None,
        "incomplete empty origin is honestly unknown"
    );
    assert!(
        !unknown.source_set_complete(),
        "unknown is not promoted to a complete empty set"
    );
    Ok(())
}

#[test]
fn field_origin_wire_rejects_duplicated_or_unbounded_reference_sets() {
    for references in [
        json!(["same", "same"]),
        json!([""]),
        json!(["x".repeat((1 << 20) + 1)]),
    ] {
        let invalid = json!({"containsEncryptedSource": null, "sourceUsesRefs": references, "sourceSetComplete": false});
        assert!(
            FieldOriginSeed::new(reader_limits())
                .deserialize(invalid)
                .is_err(),
            "invalid wire cannot restore a bounded origin"
        );
    }
}

#[test]
fn field_maps_reject_duplicate_ownership_without_overwriting_the_original() -> TestResult {
    let uses = SourceUses::new();
    let origin = uses
        .value(SourceValue::Present(()), &[], true)
        .origin()
        .clone();
    let path = FieldPath::parse("/data/payload/value")?;
    let mut fields = FieldOrigins::new();
    fields.insert(path.clone(), origin.clone())?;
    assert!(
        fields.insert(path.clone(), origin).is_err(),
        "a second owner cannot replace a field silently"
    );
    let encoded = serde_json::to_string(&fields)?;
    let decoded: FieldOrigins = FieldOriginsSeed::new(reader_limits())
        .deserialize(&mut serde_json::Deserializer::from_str(&encoded))?;
    assert_eq!(
        decoded
            .get(&path)
            .ok_or("field origin")?
            .contains_encrypted_source(),
        Some(false),
        "the immediate field marker survives without a registry recomputation"
    );
    let duplicate = r#"{"/data/value":{"containsEncryptedSource":false,"sourceUsesRefs":[],"sourceSetComplete":true},"/data/value":{"containsEncryptedSource":null,"sourceUsesRefs":[],"sourceSetComplete":false}}"#;
    assert!(
        FieldOriginsSeed::new(reader_limits())
            .deserialize(&mut serde_json::Deserializer::from_str(duplicate))
            .is_err(),
        "duplicate JSON map keys are not collapsed"
    );
    Ok(())
}

#[test]
fn wire_metadata_does_not_create_shared_runtime_authority() {
    let first = SourceUses::new();
    let cloned = first.clone();
    let other = SourceUses::new();
    assert!(
        first.is_same_inventory(&cloned),
        "only a clone retains the existing runtime owner"
    );
    assert!(
        !first.is_same_inventory(&other),
        "another inventory cannot acquire authority through equal wire IDs"
    );
}

#[test]
fn a_whole_archive_file_stays_distinct_from_entry_and_directory_domains() -> TestResult {
    use crate::{Archive, SourceInput};
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/plain.ext", b"plain"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let sources = SourceUses::new();
    let whole = sources.observe(
        SourceInput::ArchiveFile {
            input: archive.input_origin(),
            byte_length: archive.file_len(),
        },
        InputScope::Payload,
    )?;
    let entry = archive.entries().first().ok_or("plain synthetic Entry")?;
    let origin = archive.entry_origin(entry).ok_or("actual observed Entry")?;
    let payload = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    assert_archive_file_domains(&sources, &whole, &payload)?;
    assert_archive_file_source_kinds(&sources)?;
    Ok(())
}

fn assert_archive_file_domains(
    sources: &SourceUses,
    whole: &SourceScope,
    payload: &SourceScope,
) -> TestResult {
    let metadata = payload.for_scope(InputScope::ArchiveDirectoryMetadata);
    assert_ne!(
        whole.source_ref(),
        payload.source_ref(),
        "whole-file reads never acquire a leaf identity"
    );
    let at = |scope: &SourceScope, field: &str| {
        scope.at(
            SourceLocation::Metadata {
                field: field.to_owned(),
            },
            SourceUseRole::Conversion,
        )
    };
    let whole_value = sources.value(SourceValue::Present(()), &[at(whole, "sha256")?], true);
    let payload_value = sources.value(SourceValue::Present(()), &[at(payload, "bytes")?], true);
    let metadata_value = sources.value(SourceValue::Present(()), &[at(&metadata, "path")?], true);
    assert_eq!(
        whole_value.origin().contains_encrypted_source(),
        None,
        "whole-file storage has no Entry declaration"
    );
    assert_eq!(
        payload_value.origin().contains_encrypted_source(),
        Some(false),
        "the plain Entry retains its own proof"
    );
    assert_eq!(
        metadata_value.origin().contains_encrypted_source(),
        Some(false),
        "root directory facts are independently plain"
    );
    Ok(())
}

fn assert_archive_file_source_kinds(sources: &SourceUses) -> TestResult {
    let wire = serde_json::to_value(sources.freeze()?)?;
    let physical = wire
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    assert_eq!(
        physical.len(),
        2,
        "the same opaque Archive owner has two distinct actual input domains"
    );
    assert!(
        physical
            .iter()
            .any(|source| source.get("kind").and_then(Value::as_str) == Some("archiveFile")),
        "the root-file source is explicit"
    );
    assert!(
        physical
            .iter()
            .any(|source| source.get("kind").and_then(Value::as_str) == Some("archiveEntry")),
        "the Entry source remains explicit"
    );
    Ok(())
}

#[test]
fn a_root_directory_survey_cannot_be_created_from_an_opened_container() -> TestResult {
    use crate::{Archive, SourceLocation};
    use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let nested = TestContainer::new()
        .entry(TestEntry::stored("child.txt", b"content"))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("nested.socpak", nested))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive
        .entries()
        .first()
        .ok_or("original container Entry")?;
    let container = archive.open_container(entry)?;
    let sources = SourceUses::new();
    assert!(
        sources.observe_root_directory_paths(&container)?.is_none(),
        "a child directory uses its container payload's ancestry"
    );
    let root = sources
        .observe_root_directory_paths(&archive)?
        .ok_or("successfully parsed root directory")?;
    assert!(matches!(
        root.description().location(),
        SourceLocation::ArchiveDirectoryPaths { start_entry_index, end_entry_index }
            if start_entry_index.get() == 0 && end_entry_index.get() == 1
    ));
    Ok(())
}
