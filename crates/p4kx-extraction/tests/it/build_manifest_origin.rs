// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Completed build-manifest inputs share the standalone-file provenance contract.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::slice;

use p4kx_extraction::{
    Archive, BuildManifestOrigin, DataForgeEntry, DataForgeOrigin, InputScope, SourceDetails,
    SourceEncryptionState, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses, SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, write_build_manifest};
use serde_json::Value;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn open(folder: &Path) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    if !path.exists() {
        ArchiveBuilder::new()
            .entry(TestEntry::stored("encrypted.bin", b"fact").encrypted())
            .write_to(&path)?;
        write_build_manifest(folder, "branch", "1.2.3", "456", "synthetic-build")?;
    }
    Ok(Archive::open(&path)?)
}

fn manifest_origin(archive: &Archive) -> TestResult<&BuildManifestOrigin> {
    Ok(archive
        .build_manifest_observation()
        .origin()
        .ok_or("original completed manifest read")?)
}

fn assert_unknown_origin(value: &Value) {
    assert_eq!(
        value.get("containsEncryptedSource"),
        Some(&Value::Null),
        "Unknown is an explicit null marker"
    );
    assert_eq!(
        value.get("sourceSetComplete"),
        Some(&Value::Bool(true)),
        "coordinate completeness does not imply known encryption"
    );
    assert_eq!(
        value
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "the immediate value retains its actual contributing use"
    );
}

#[test]
fn manifest_clones_deduplicate_but_equal_file_reads_keep_their_own_identity() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = open(directory.path())?;
    let reopened = open(directory.path())?;
    assert_eq!(
        archive.build(),
        reopened.build(),
        "equal parsed values do not unify inputs"
    );
    let original_origin = manifest_origin(&archive)?;
    let cloned = original_origin.clone();
    let read = DataForgeEntry::read_file(&directory.path().join(original_origin.name()))?;
    let DataForgeOrigin::File(dataforge) = read.origin() else {
        return Err("standalone original file read".into());
    };
    let sources = SourceUses::new();
    let original = sources.observe(
        SourceInput::BuildManifest(original_origin),
        InputScope::Payload,
    )?;
    let duplicate = sources.observe(SourceInput::BuildManifest(&cloned), InputScope::Payload)?;
    let independent = sources.observe(
        SourceInput::BuildManifest(manifest_origin(&reopened)?),
        InputScope::Payload,
    )?;
    let standalone = sources.observe(SourceInput::File(dataforge), InputScope::Payload)?;

    assert_eq!(
        original.source_ref(),
        duplicate.source_ref(),
        "a clone deduplicates by original read identity"
    );
    assert_ne!(
        original.source_ref(),
        independent.source_ref(),
        "an independent manifest read stays distinct"
    );
    assert_ne!(
        original.source_ref(),
        standalone.source_ref(),
        "a standalone read is not the manifest read"
    );
    assert_ne!(
        independent.source_ref(),
        standalone.source_ref(),
        "matching path and bytes grant no source identity"
    );
    let inventory = sources.freeze()?;
    assert_eq!(
        inventory.sources().len(),
        3,
        "only genuine clones deduplicate"
    );
    let length = original_origin.byte_length().to_string();
    for source in inventory.sources() {
        assert_file_source(&serde_json::to_value(source)?, &length)?;
    }
    Ok(())
}

#[test]
fn manifest_domains_remain_unknown_under_detail_limits_and_archive_encryption() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = open(directory.path())?;
    let observed = manifest_origin(&archive)?;
    let length = observed.byte_length().to_string();
    fs::remove_file(directory.path().join(observed.name()))?;
    let sources = SourceUses::new();
    let scope = sources.observe_with_details(
        SourceInput::BuildManifest(observed),
        InputScope::Payload,
        SourceDetails::CaptureLimited,
    )?;
    for domain in [
        InputScope::Payload,
        InputScope::FileMetadata,
        InputScope::ArchiveDirectoryMetadata,
    ] {
        assert_domain(&sources, &scope, domain)?;
    }
    let inventory = sources.freeze()?;
    assert_eq!(
        inventory.sources().len(),
        1,
        "all byte domains share the one original file"
    );
    let source = serde_json::to_value(inventory.sources().first().ok_or("manifest source")?)?;
    assert_eq!(
        source.pointer("/byteLength/value").and_then(Value::as_str),
        Some(length.as_str()),
        "the registry does not reopen the deleted manifest"
    );
    assert_eq!(
        source.get("kind").and_then(Value::as_str),
        Some("file"),
        "detail limits preserve original external-file identity"
    );
    assert!(
        source
            .get("ancestors")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "no inferred Archive ancestry is added"
    );
    Ok(())
}

#[test]
fn manifest_uses_never_transfer_authority_between_preparation_inventories() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = open(directory.path())?;
    let original_origin = manifest_origin(&archive)?;
    let own = SourceUses::new();
    let foreign = SourceUses::new();
    let own_scope = own.observe(
        SourceInput::BuildManifest(original_origin),
        InputScope::Payload,
    )?;
    let foreign_scope = foreign.observe(
        SourceInput::BuildManifest(original_origin),
        InputScope::Payload,
    )?;
    let location = SourceLocation::Metadata {
        field: "BuildId".to_owned(),
    };
    let own_use = own_scope.at(location.clone(), SourceUseRole::Payload)?;
    let foreign_use = foreign_scope.at(location, SourceUseRole::Payload)?;
    assert_eq!(
        own_use.reference(),
        foreign_use.reference(),
        "matching serialized references do not grant ownership"
    );
    assert_foreign_value(&own, &foreign_use)?;
    assert_eq!(
        own.freeze()?.sources().len(),
        1,
        "local physical identity remains local"
    );
    assert_eq!(
        foreign.freeze()?.sources().len(),
        1,
        "the foreign owner keeps its own source"
    );
    Ok(())
}

#[test]
fn a_malformed_empty_manifest_registers_a_present_zero_byte_file() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = open(directory.path())?;
    let manifest_path = directory.path().join("build_manifest.id");
    fs::write(&manifest_path, b"")?;
    let malformed = Archive::open(archive.path())?;
    assert!(
        malformed.build().is_none(),
        "the empty original read has no parsed Build"
    );
    let original = manifest_origin(&malformed)?;
    assert_eq!(
        original.byte_length(),
        0,
        "an empty completed read still has an original owner"
    );
    write_build_manifest(directory.path(), "later", "7.8.9", "999", "later-build")?;

    let sources = SourceUses::new();
    sources.observe(SourceInput::BuildManifest(original), InputScope::Payload)?;
    let inventory = sources.freeze()?;
    assert_eq!(
        inventory.sources().len(),
        1,
        "the malformed read is a real standalone input"
    );
    let source = serde_json::to_value(inventory.sources().first().ok_or("empty manifest source")?)?;
    let length = source.get("byteLength").ok_or("byte-length datum")?;
    assert_eq!(
        length.get("state").and_then(Value::as_str),
        Some("present"),
        "zero is present, not absent"
    );
    assert_eq!(
        length.get("value").and_then(Value::as_str),
        Some("0"),
        "the registry retains the original empty read"
    );
    assert_unknown_origin(length);
    Ok(())
}

fn assert_file_source(wire: &Value, length: &str) -> TestResult {
    assert_eq!(
        wire.get("kind").and_then(Value::as_str),
        Some("file"),
        "all external files share the existing wire kind"
    );
    assert_eq!(
        wire.get("storedEncryption").and_then(Value::as_str),
        Some("unknown"),
        "the manifest grants no original P4K lineage proof"
    );
    assert_eq!(
        wire.pointer("/name/value").and_then(Value::as_str),
        Some("build_manifest.id"),
        "no host directory is serialized"
    );
    assert_eq!(
        wire.pointer("/byteLength/value").and_then(Value::as_str),
        Some(length),
        "the completed read supplies the exact decimal byte length"
    );
    assert!(
        wire.get("ancestors")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "external reads acquire no Archive ancestors"
    );
    assert!(wire.get("path").is_none(), "host path is not published");
    assert_unknown_origin(wire.get("name").ok_or("file name")?);
    assert_unknown_origin(wire.get("byteLength").ok_or("file byte length")?);
    Ok(())
}

fn assert_domain(sources: &SourceUses, scope: &SourceScope, domain: InputScope) -> TestResult {
    let used = scope.for_scope(domain).at(
        SourceLocation::Metadata {
            field: "fact".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    assert_eq!(
        used.encryption(),
        SourceEncryptionState::Unknown,
        "an encrypted Archive Entry cannot taint its separate manifest"
    );
    let value = sources.value(SourceValue::Present(false), slice::from_ref(&used), true);
    assert_eq!(
        value.value(),
        Some(&false),
        "a false payload remains present"
    );
    assert_eq!(
        value.origin().source_uses_refs(),
        slice::from_ref(used.reference()),
        "the immediate marker cites the original use"
    );
    let wire = serde_json::to_value(value)?;
    assert_unknown_origin(&wire);
    Ok(())
}

fn assert_foreign_value(own: &SourceUses, foreign_use: &SourceUse) -> TestResult {
    let value = own.value(
        SourceValue::Present("value"),
        slice::from_ref(foreign_use),
        true,
    );
    assert_eq!(
        value.value(),
        Some(&"value"),
        "foreign evidence never rewrites the value"
    );
    assert!(
        !value.origin().source_set_complete(),
        "a foreign contribution makes the local set incomplete"
    );
    assert!(
        value.origin().source_uses_refs().is_empty(),
        "foreign evidence is never adopted"
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        None,
        "ownership failure stays Unknown"
    );
    let wire = serde_json::to_value(value)?;
    assert_eq!(
        wire.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the marker key is explicitly present"
    );
    assert_eq!(
        wire.get("sourceSetComplete"),
        Some(&Value::Bool(false)),
        "the wire retains the incomplete contribution set"
    );
    Ok(())
}
