// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::consistency_mutation as changes;
use super::mutation::Snapshot;
use super::support::{Fixture, Scene, TestResult};
use crate::MediaError;
use crate::origin_inventory_wire::Encryption;

const PHYSICAL: &str = "captured source-use encryption contradicts physical source evidence";
const DECLARATIONS: &str = "stored source encryption contradicts available directory declarations";
const BYTES: &str = "raw declaration word disagrees with available raw bytes";
const DECLARATION: &str = "/declarations/value/0/value";

#[test]
fn available_encrypted_raw_declarations_prevent_relabelling_stored_state_and_uses() -> TestResult {
    for state in [Encryption::Unencrypted, Encryption::Unknown] {
        let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
        let mut snapshot = checked(&fixture)?;
        changes::relabel_stored(&mut snapshot, state)?;
        reject(&mut fixture, &mut snapshot, DECLARATIONS)?;
    }
    Ok(())
}

#[test]
fn available_plain_raw_declarations_prevent_invented_encrypted_or_unknown_states() -> TestResult {
    for state in [Encryption::Encrypted, Encryption::Unknown] {
        let mut fixture = Fixture::new(Scene::Plain)?;
        let mut snapshot = checked(&fixture)?;
        changes::relabel_stored(&mut snapshot, state)?;
        reject(&mut fixture, &mut snapshot, DECLARATIONS)?;
    }
    Ok(())
}

#[test]
fn raw_word_and_zero_one_or_two_available_bytes_cannot_disagree() -> TestResult {
    for bytes in [vec![], vec![1], vec![0, 0], vec![1, 0, 0]] {
        let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
        let mut snapshot = checked(&fixture)?;
        changes::replace(
            &mut snapshot,
            &format!("{DECLARATION}/rawBytes/value"),
            serde_json::to_value(bytes)?,
        )?;
        reject(&mut fixture, &mut snapshot, BYTES)?;
    }
    let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
    let mut snapshot = checked(&fixture)?;
    changes::replace(
        &mut snapshot,
        &format!("{DECLARATION}/rawWord/value"),
        Value::from(0),
    )?;
    reject(&mut fixture, &mut snapshot, BYTES)
}

#[test]
fn zip_flags_record_format_and_declaration_shape_constrain_the_same_stored_fact() -> TestResult {
    for (pointer, value) in [
        ("/zipFlags/value", Value::from(1)),
        ("/zipFlags/value", Value::from(64)),
        ("/recordFormat", Value::String("unknown".into())),
        (
            "/declarations/value/0/value/kind",
            Value::String("unknown".into()),
        ),
        (
            "/declarations/value/0/value/kind",
            Value::String("p4kV2Field".into()),
        ),
        (
            "/declarations/value/0/value/declaredBlockLength/value",
            Value::from(5),
        ),
    ] {
        let mut fixture = Fixture::new(Scene::Plain)?;
        let mut snapshot = checked(&fixture)?;
        changes::replace(&mut snapshot, pointer, value)?;
        reject(&mut fixture, &mut snapshot, DECLARATIONS)?;
    }
    Ok(())
}

#[test]
fn supported_unknown_declarations_cannot_be_relabelled_definite() -> TestResult {
    for builder in unknown_builders() {
        for state in [Encryption::Encrypted, Encryption::Unencrypted] {
            let mut fixture = Fixture::from_builder(&builder)?;
            let mut snapshot = checked(&fixture)?;
            let source = snapshot.source(super::mutation::Target::Leaf)?;
            assert_eq!(
                source.get("storedEncryption").and_then(Value::as_str),
                Some("unknown"),
                "malformed, unsupported or conflicting actual declarations remain unknown"
            );
            changes::relabel_stored(&mut snapshot, state)?;
            reject(&mut fixture, &mut snapshot, DECLARATIONS)?;
        }
    }
    Ok(())
}

#[test]
fn transitive_encrypted_grandparents_survive_a_cropped_leaf_ancestor_list() -> TestResult {
    for scene in [
        Scene::EncryptedGrandparent,
        Scene::EncryptedOuterUnknownAncestor,
    ] {
        for forged in [Encryption::Unencrypted, Encryption::Unknown] {
            let mut fixture = Fixture::new(scene)?;
            let mut snapshot = checked(&fixture)?;
            changes::crop_outer(&mut snapshot, Encryption::Encrypted, forged)?;
            reject(&mut fixture, &mut snapshot, PHYSICAL)?;
        }
    }
    Ok(())
}

#[test]
fn transitive_unknown_grandparents_cannot_be_cropped_into_complete_plain_evidence() -> TestResult {
    let mut fixture = Fixture::new(Scene::UnknownGrandparent)?;
    let mut snapshot = checked(&fixture)?;
    changes::crop_outer(&mut snapshot, Encryption::Unknown, Encryption::Unencrypted)?;
    reject(&mut fixture, &mut snapshot, PHYSICAL)
}

#[test]
fn canonical_plain_encrypted_and_unknown_directory_facts_remain_reusable() -> TestResult {
    let mut builders = unknown_builders();
    builders.extend([
        ArchiveBuilder::new().entry(TestEntry::stored("leaf.ext", b"original synthetic bytes")),
        ArchiveBuilder::v2().entry(TestEntry::stored("leaf.ext", b"original synthetic bytes")),
        ArchiveBuilder::v2()
            .entry(TestEntry::zstd("leaf.ext", b"original synthetic bytes").encrypted()),
    ]);
    for builder in builders {
        let fixture = Fixture::from_builder(&builder)?;
        let _snapshot = checked(&fixture)?;
    }
    Ok(())
}

#[test]
fn unavailable_optional_full_details_preserve_known_encrypted_minimal_evidence() -> TestResult {
    for minimal in [false, true] {
        let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
        let mut snapshot = checked(&fixture)?;
        if minimal {
            changes::minimal(&mut snapshot)?;
        } else {
            changes::fact(&mut snapshot, "/declarations", None)?;
            *snapshot
                .sources
                .iter_mut()
                .find(|source| {
                    source.pointer("/path/value").and_then(Value::as_str) == Some("leaf.ext")
                })
                .and_then(|source| source.pointer_mut("/declarations/state"))
                .ok_or("the original declarations fact state")? =
                Value::String("unreadable".into());
        }
        snapshot.publish_forgery(&mut fixture)?;
        assert!(
            fixture.reuse()?.is_some(),
            "absence of optional details cannot erase known original proof"
        );
    }
    Ok(())
}

fn unknown_builders() -> Vec<ArchiveBuilder> {
    [
        TestEntry::zstd("leaf.ext", b"original synthetic bytes")
            .encrypted()
            .with_encryption_field(7),
        TestEntry::zstd("leaf.ext", b"original synthetic bytes")
            .encrypted()
            .with_encryption_block(Vec::new()),
        TestEntry::zstd("leaf.ext", b"original synthetic bytes")
            .encrypted()
            .with_encryption_block(vec![1]),
        TestEntry::zstd("leaf.ext", b"original synthetic bytes")
            .encrypted()
            .with_encryption_field(0)
            .with_empty_encryption_blocks(1),
        TestEntry::zstd("leaf.ext", b"original synthetic bytes")
            .encrypted()
            .with_empty_encryption_blocks(1),
    ]
    .into_iter()
    .map(|entry| ArchiveBuilder::new().entry(entry))
    .collect()
}

fn checked(fixture: &Fixture) -> Result<Snapshot, Box<dyn std::error::Error>> {
    assert!(
        fixture.reuse()?.is_some(),
        "the genuine producer generation is a reusable positive control"
    );
    Snapshot::load(fixture)
}

fn reject(fixture: &mut Fixture, snapshot: &mut Snapshot, expected: &str) -> TestResult {
    snapshot.publish_forgery(fixture)?;
    let outcome = fixture.reuse();
    assert!(
        matches!(&outcome, Err(MediaError::Destination(detail)) if detail == expected),
        "fully rebound generation must fail the intended {expected} consistency check"
    );
    Ok(())
}
