// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Captured use evidence across physical inputs and byte domains.

use std::error::Error;
use std::fs;
use std::slice;

use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, InputScope, SourceDetails, SourceEncryptionState, SourceInput,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourceValueState,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use crate::source_minimal_proof as support;

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) fn used(scope: &SourceScope, role: SourceUseRole) -> TestResult<SourceUse> {
    Ok(scope.at(
        SourceLocation::Metadata {
            field: "fact".into(),
        },
        role,
    )?)
}

pub(super) fn assert_evidence(
    sources: &SourceUses,
    used: &SourceUse,
    expected: SourceEncryptionState,
) -> TestResult {
    assert_eq!(
        used.encryption(),
        expected,
        "the runtime byte domain is captured"
    );
    let row = serde_json::to_value(used.description())?;
    let expected = serde_json::to_value(expected)?;
    assert_eq!(
        row.get("effectiveEncryption"),
        Some(&expected),
        "the actual use serializer retains its captured domain evidence"
    );
    assert_eq!(
        row.get("id").and_then(Value::as_str),
        Some(used.reference().as_str()),
        "serialized evidence retains the original use identity"
    );
    let frozen = sources.freeze()?;
    let retained = frozen
        .uses()
        .iter()
        .find(|row| row.id() == used.reference())
        .ok_or("original frozen use")?;
    assert_eq!(
        serde_json::to_value(retained)?,
        row,
        "freeze preserves the original description and captured evidence"
    );
    Ok(())
}

pub(super) fn assert_marker(
    sources: &SourceUses,
    used: &SourceUse,
    marker: Option<bool>,
    complete: bool,
) {
    let value = sources.value(SourceValue::Present(0_u32), slice::from_ref(used), true);
    assert_eq!(
        value.state(),
        SourceValueState::Present,
        "zero stays present"
    );
    assert_eq!(
        value.value(),
        Some(&0),
        "origin capture preserves the payload"
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        marker,
        "the marker follows the actual byte domain"
    );
    assert_eq!(
        value.origin().source_set_complete(),
        complete,
        "coordinate completeness is independent of encryption"
    );
    assert_eq!(
        value.origin().source_uses_refs(),
        slice::from_ref(used.reference()),
        "the direct contribution references the actual original use"
    );
}

pub(super) fn assert_domains(
    sources: &SourceUses,
    scope: &SourceScope,
    payload: SourceEncryptionState,
    payload_marker: Option<bool>,
    metadata: SourceEncryptionState,
    metadata_marker: Option<bool>,
) -> TestResult {
    let cases = [
        (InputScope::Payload, payload, payload_marker),
        (
            InputScope::ArchiveDirectoryMetadata,
            metadata,
            metadata_marker,
        ),
        (InputScope::FileMetadata, metadata, metadata_marker),
    ];
    for (domain, expected, marker) in cases {
        let used = used(&scope.for_scope(domain), SourceUseRole::Payload)?;
        assert_evidence(sources, &used, expected)?;
        assert_marker(sources, &used, marker, true);
    }
    Ok(())
}

#[test]
fn entry_domains_capture_own_declarations_independently_of_detail_retention() -> TestResult {
    use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};
    for (word, payload, marker) in [
        (0, Unencrypted, Some(false)),
        (1, Encrypted, Some(true)),
        (7, Unknown, None),
    ] {
        let (_directory, archive) = support::open(
            &ArchiveBuilder::new()
                .entry(TestEntry::stored("fact.bin", b"fact").with_encryption_field(word)),
        )?;
        for details in [SourceDetails::Full, SourceDetails::CaptureLimited] {
            let sources = SourceUses::new();
            let scope = support::source(&sources, &archive, details)?;
            assert_domains(&sources, &scope, payload, marker, Unencrypted, Some(false))?;
            let inventory = serde_json::to_value(sources.freeze()?)?;
            for row in inventory["uses"].as_array().ok_or("actual use rows")? {
                assert!(
                    row.get("effectiveEncryption")
                        .and_then(Value::as_str)
                        .is_some(),
                    "automatic metadata uses also retain captured evidence"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn standalone_and_whole_archive_file_uses_keep_unknown_domains() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.dcb");
    fs::write(&path, b"synthetic file bytes")?;
    let read = DataForgeEntry::read_file(&path)?;
    let DataForgeOrigin::File(origin) = read.origin() else {
        return Err("actual standalone file origin".into());
    };
    let sources = SourceUses::new();
    let file = sources.observe(SourceInput::File(origin), InputScope::Payload)?;
    let (_archive_directory, archive) = support::open(
        &ArchiveBuilder::new().entry(TestEntry::stored("fact.bin", b"fact").encrypted()),
    )?;
    let completed_bytes = fs::read(archive.path())?;
    let completed_length = u64::try_from(completed_bytes.len())?;
    let archive_file = sources.observe(
        SourceInput::ArchiveFile {
            input: archive.input_origin(),
            byte_length: completed_length,
        },
        InputScope::FileMetadata,
    )?;
    for scope in [file, archive_file] {
        for domain in [
            InputScope::Payload,
            InputScope::ArchiveDirectoryMetadata,
            InputScope::FileMetadata,
        ] {
            let used = used(&scope.for_scope(domain), SourceUseRole::Payload)?;
            assert_evidence(&sources, &used, SourceEncryptionState::Unknown)?;
            assert_marker(&sources, &used, None, true);
        }
    }
    Ok(())
}

#[test]
fn captured_evidence_preserves_all_source_value_states_and_payloads() -> TestResult {
    let (_directory, archive) = support::open(
        &ArchiveBuilder::new().entry(TestEntry::stored("fact.bin", b"fact").encrypted()),
    )?;
    let sources = SourceUses::new();
    let scope = support::source(&sources, &archive, SourceDetails::CaptureLimited)?;
    let used = used(&scope, SourceUseRole::Selector)?;
    assert_evidence(&sources, &used, SourceEncryptionState::Encrypted)?;
    for (source, state, payload) in [
        (
            SourceValue::Present(0_u32),
            SourceValueState::Present,
            json!(0),
        ),
        (SourceValue::Absent, SourceValueState::Absent, Value::Null),
        (SourceValue::Null, SourceValueState::Null, Value::Null),
        (
            SourceValue::Unreadable,
            SourceValueState::Unreadable,
            Value::Null,
        ),
        (
            SourceValue::Unsupported,
            SourceValueState::Unsupported,
            Value::Null,
        ),
        (SourceValue::Invalid, SourceValueState::Invalid, Value::Null),
    ] {
        assert_value_state(&sources, &used, source, state, &payload)?;
    }
    Ok(())
}

fn assert_value_state(
    sources: &SourceUses,
    used: &SourceUse,
    source: SourceValue<u32>,
    state: SourceValueState,
    payload: &Value,
) -> TestResult {
    let value = sources.value(source, slice::from_ref(used), true);
    assert_eq!(
        value.state(),
        state,
        "origin capture preserves the source-value state"
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(true),
        "the actual encrypted selector contributes in every value state"
    );
    assert!(
        value.origin().source_set_complete(),
        "the retained original selector has a complete contribution set"
    );
    assert_eq!(
        value.origin().source_uses_refs(),
        slice::from_ref(used.reference()),
        "the direct contribution references the actual original use"
    );
    let wire = serde_json::to_value(value)?;
    assert_eq!(
        wire.get("state"),
        Some(&serde_json::to_value(state)?),
        "the serializer preserves the actual state"
    );
    assert_eq!(
        wire.get("value"),
        Some(payload),
        "origin evidence does not rewrite the payload"
    );
    Ok(())
}
