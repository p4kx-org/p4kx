// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Immutable use evidence through ancestors, detail upgrades and shared owners.

use std::ptr;
use std::slice;

use p4kx_extraction::{
    Archive, InputScope, SourceCoordinateOmission, SourceDetails, SourceEncryptionState,
    SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses, SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Error as JsonError, Value};

use crate::source_minimal_proof as support;
use crate::source_use_encryption as evidence;
use crate::source_use_encryption::TestResult;

#[test]
fn unknown_inputs_and_actual_unknown_ancestors_retain_distinct_metadata_evidence() -> TestResult {
    use SourceEncryptionState::{Unencrypted, Unknown};
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"fact"))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("room.pak", leaf)))?;
    let detached = support::first(&root)?.clone();
    let child = root.open_container(&detached)?;
    let sources = SourceUses::new();
    let leaf = support::source(&sources, &child, SourceDetails::CaptureLimited)?;
    let ancestor = leaf.ancestor(0)?.ok_or("actual unknown containing Entry")?;
    let unknown = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    evidence::assert_domains(&sources, &unknown, Unknown, None, Unknown, None)?;
    evidence::assert_domains(&sources, &ancestor, Unknown, None, Unencrypted, Some(false))?;
    evidence::assert_domains(&sources, &leaf, Unknown, None, Unknown, None)?;
    assert_ne!(unknown.source_ref(), ancestor.source_ref());
    let inventory = serde_json::to_value(sources.freeze()?)?;
    assert_unknown_kind(&inventory, &unknown)?;
    assert_unknown_kind(&inventory, &ancestor)?;
    Ok(())
}

#[test]
fn actual_plain_and_encrypted_ancestors_keep_their_original_byte_domains() -> TestResult {
    use SourceEncryptionState::{Encrypted, Unencrypted};
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"fact"))
        .build();
    let middle = ArchiveBuilder::new()
        .entry(TestEntry::deflated("inner.pak", leaf).encrypted())
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("outer.pak", middle)))?;
    let outer = root.open_container(support::first(&root)?)?;
    let inner = outer.open_container(support::first(&outer)?)?;
    let sources = SourceUses::new();
    let leaf = support::source(&sources, &inner, SourceDetails::CaptureLimited)?;
    let plain = leaf.ancestor(0)?.ok_or("plain original ancestor")?;
    let encrypted = leaf.ancestor(1)?.ok_or("encrypted original ancestor")?;
    evidence::assert_domains(
        &sources,
        &plain,
        Unencrypted,
        Some(false),
        Unencrypted,
        Some(false),
    )?;
    evidence::assert_domains(
        &sources,
        &encrypted,
        Encrypted,
        Some(true),
        Unencrypted,
        Some(false),
    )?;
    evidence::assert_domains(
        &sources,
        &leaf,
        Encrypted,
        Some(true),
        Encrypted,
        Some(true),
    )?;
    Ok(())
}

#[test]
fn detail_upgrades_share_original_descriptions_for_every_captured_state() -> TestResult {
    use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};
    for (word, expected) in [(0, Unencrypted), (1, Encrypted), (7, Unknown)] {
        let (_directory, archive) = support::open(
            &ArchiveBuilder::new()
                .entry(TestEntry::stored("fact.bin", b"fact").with_encryption_field(word)),
        )?;
        check_upgrade(&archive, expected, Unencrypted)?;
    }
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"fact"))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("room.pak", leaf)))?;
    let detached = support::first(&root)?.clone();
    let child = root.open_container(&detached)?;
    check_upgrade(&child, Unknown, Unknown)
}

fn path_use(scope: &SourceScope) -> TestResult<SourceUse> {
    Ok(scope.for_scope(InputScope::ArchiveDirectoryMetadata).at(
        SourceLocation::Metadata {
            field: "path".into(),
        },
        SourceUseRole::Payload,
    )?)
}

fn check_upgrade(
    archive: &Archive,
    payload: SourceEncryptionState,
    metadata: SourceEncryptionState,
) -> TestResult {
    let sources = SourceUses::new();
    let sparse = support::source(&sources, archive, SourceDetails::CaptureLimited)?;
    let originals = [
        evidence::used(&sparse, SourceUseRole::Payload)?,
        path_use(&sparse)?,
    ];
    let bytes = description_bytes(&originals)?;
    let full = support::source(&sources, archive, SourceDetails::Full)?;
    assert_eq!(
        sparse.source_ref(),
        full.source_ref(),
        "detail upgrades retain the original physical source"
    );
    let repeated = [
        evidence::used(&full, SourceUseRole::Payload)?,
        path_use(&full)?,
    ];
    let pairs = originals.iter().zip(&repeated).zip([payload, metadata]);
    for ((original, repeated), expected) in pairs {
        assert!(
            ptr::eq(original.description(), repeated.description()),
            "repeated observations share their original description"
        );
        assert!(
            ptr::eq(original.description(), original.clone().description()),
            "a cloned handle borrows the same original description"
        );
        assert_eq!(
            original.reference(),
            repeated.reference(),
            "repeated observations retain their stable use identity"
        );
        evidence::assert_evidence(&sources, original, expected)?;
        evidence::assert_evidence(&sources, repeated, expected)?;
    }
    assert_eq!(
        bytes,
        description_bytes(&originals)?,
        "an upgrade never mutates an existing use row"
    );
    let before = serde_json::to_vec(&sources.freeze()?)?;
    let limited = support::source(&sources, archive, SourceDetails::CaptureLimited)?;
    assert_eq!(
        full.source_ref(),
        limited.source_ref(),
        "limited revisits retain the original physical source"
    );
    let _payload = evidence::used(&limited, SourceUseRole::Payload)?;
    let _metadata = path_use(&limited)?;
    assert_eq!(
        before,
        serde_json::to_vec(&sources.freeze()?)?,
        "no downgrade or rebinding"
    );
    Ok(())
}

#[test]
fn opaque_uses_retain_exact_evidence_without_claiming_complete_coordinates() -> TestResult {
    use SourceEncryptionState::{Encrypted, Unencrypted, Unknown};
    for (word, expected, marker) in [
        (0, Unencrypted, None),
        (1, Encrypted, Some(true)),
        (7, Unknown, None),
    ] {
        let (_directory, archive) = support::open(
            &ArchiveBuilder::new()
                .entry(TestEntry::stored("fact.bin", b"fact").with_encryption_field(word)),
        )?;
        let sources = SourceUses::new();
        let scope = support::source(&sources, &archive, SourceDetails::CaptureLimited)?;
        let actual = evidence::used(&scope, SourceUseRole::Conversion)?;
        let opaque = sources.opaque_use(&actual, SourceCoordinateOmission::CaptureLimit)?;
        let repeated = scope.at_opaque(
            SourceUseRole::Conversion,
            SourceCoordinateOmission::CaptureLimit,
        )?;
        evidence::assert_evidence(&sources, &opaque, expected)?;
        evidence::assert_evidence(&sources, &repeated, expected)?;
        assert!(ptr::eq(opaque.description(), repeated.description()));
        assert_eq!(opaque.reference(), repeated.reference());
        assert_eq!(opaque.description().source(), actual.description().source());
        assert_eq!(opaque.description().scope(), actual.description().scope());
        assert_eq!(opaque.role(), actual.role());
        assert!(matches!(
            opaque.description().location(),
            SourceLocation::Opaque { .. }
        ));
        evidence::assert_marker(&sources, &opaque, marker, false);
    }
    Ok(())
}

#[test]
fn foreign_owners_and_ignored_roles_keep_their_encryption_boundary() -> TestResult {
    let (_directory, archive) = support::open(
        &ArchiveBuilder::new().entry(TestEntry::stored("fact.bin", b"fact").encrypted()),
    )?;
    let sources = SourceUses::new();
    let scope = support::source(&sources, &archive, SourceDetails::CaptureLimited)?;
    let local = evidence::used(&scope, SourceUseRole::Payload)?;
    let other = SourceUses::new();
    let foreign_scope = support::source(&other, &archive, SourceDetails::CaptureLimited)?;
    let foreign = evidence::used(&foreign_scope, SourceUseRole::Payload)?;
    assert_eq!(
        local.reference(),
        foreign.reference(),
        "same spelling grants no ownership"
    );
    assert!(!ptr::eq(local.description(), foreign.description()));
    assert!(matches!(
        sources.opaque_use(&foreign, SourceCoordinateOmission::CaptureLimit),
        Err(SourceEvidenceError::ForeignEvidence)
    ));
    let excluded = sources.value(SourceValue::Present(0_u32), slice::from_ref(&foreign), true);
    assert_eq!(excluded.value(), Some(&0));
    assert_eq!(excluded.origin().contains_encrypted_source(), None);
    assert!(!excluded.origin().source_set_complete());
    assert_eq!(
        excluded.origin().source_uses_refs(),
        [],
        "foreign handles never become actual contributors"
    );
    evidence::assert_evidence(&sources, &local, SourceEncryptionState::Encrypted)?;
    evidence::assert_evidence(&other, &foreign, SourceEncryptionState::Encrypted)?;
    let mut ignored = Vec::new();
    for role in [SourceUseRole::Attempted, SourceUseRole::Reference] {
        let observed = scope.at_opaque(role, SourceCoordinateOmission::CaptureLimit)?;
        evidence::assert_evidence(&sources, &observed, SourceEncryptionState::Encrypted)?;
        ignored.push(observed);
    }
    let literal = sources.value(SourceValue::Present(false), &ignored, true);
    assert_eq!(literal.value(), Some(&false));
    assert_eq!(literal.origin().contains_encrypted_source(), Some(false));
    assert!(literal.origin().source_set_complete());
    assert_eq!(
        literal.origin().source_uses_refs(),
        [],
        "attempted and reference observations do not contribute to the literal"
    );
    let kept = sources.value(SourceValue::Present(0_u32), &[local], true);
    assert_eq!(kept.origin().contains_encrypted_source(), Some(true));
    Ok(())
}

fn description_bytes(uses: &[SourceUse]) -> Result<Vec<Vec<u8>>, JsonError> {
    uses.iter()
        .map(|used| serde_json::to_vec(used.description()))
        .collect()
}

fn assert_unknown_kind(inventory: &Value, scope: &SourceScope) -> TestResult {
    let row = inventory["sources"]
        .as_array()
        .ok_or("physical rows")?
        .iter()
        .find(|row| row["id"] == scope.source_ref().as_str())
        .ok_or("unknown row")?;
    assert_eq!(
        row.get("kind").and_then(Value::as_str),
        Some("unknown"),
        "the physical kind remains explicitly unknown"
    );
    assert_eq!(
        row.get("storedEncryption").and_then(Value::as_str),
        Some("unknown"),
        "the unknown physical row does not invent a stored declaration"
    );
    Ok(())
}
