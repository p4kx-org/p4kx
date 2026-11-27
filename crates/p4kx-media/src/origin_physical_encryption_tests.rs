// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "origin_physical_encryption_mutation.rs"]
mod mutation;
#[path = "origin_physical_encryption_support.rs"]
mod support;

use crate::MediaError;
use crate::origin_inventory_wire::{Encryption, Scope};
use mutation::{Snapshot, Target, UseSet, Witness};
use support::{Fixture, Scene, TestResult};

const CONTRADICTION: &str = "captured source-use encryption contradicts physical source evidence";

#[test]
fn known_encrypted_leaf_payload_cannot_be_relabelled_plain_or_unknown() -> TestResult {
    for forged in [Encryption::Unencrypted, Encryption::Unknown] {
        reject(
            Scene::EncryptedLeaf,
            Witness {
                target: Target::Leaf,
                scope: Scope::Payload,
                captured: Encryption::Encrypted,
                forged,
                use_set: UseSet::FieldContributor,
            },
        )?;
    }
    Ok(())
}

#[test]
fn encrypted_ancestors_constrain_both_domains_across_unknown_middle_entries() -> TestResult {
    for scene in [
        Scene::EncryptedAncestor,
        Scene::EncryptedOuterUnknownAncestor,
    ] {
        for scope in [Scope::Payload, Scope::ArchiveDirectoryMetadata] {
            for forged in [Encryption::Unencrypted, Encryption::Unknown] {
                reject(
                    scene,
                    Witness {
                        target: Target::Leaf,
                        scope,
                        captured: Encryption::Encrypted,
                        forged,
                        use_set: UseSet::FieldContributor,
                    },
                )?;
            }
        }
    }
    Ok(())
}

#[test]
fn determinate_plain_and_unknown_entry_domains_reject_other_captured_states() -> TestResult {
    for (scene, captured, forged) in [
        (Scene::Plain, Encryption::Unencrypted, Encryption::Encrypted),
        (Scene::Plain, Encryption::Unencrypted, Encryption::Unknown),
        (
            Scene::UnknownAncestor,
            Encryption::Unknown,
            Encryption::Unencrypted,
        ),
        (
            Scene::UnknownAncestor,
            Encryption::Unknown,
            Encryption::Encrypted,
        ),
    ] {
        for scope in [Scope::Payload, Scope::ArchiveDirectoryMetadata] {
            reject(
                scene,
                Witness {
                    target: Target::Leaf,
                    scope,
                    captured,
                    forged,
                    use_set: UseSet::FieldContributor,
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn unused_reference_descriptions_cannot_contradict_a_known_domain() -> TestResult {
    for (scope, captured, forged) in [
        (
            Scope::Payload,
            Encryption::Encrypted,
            Encryption::Unencrypted,
        ),
        (
            Scope::ArchiveDirectoryMetadata,
            Encryption::Unencrypted,
            Encryption::Encrypted,
        ),
        (
            Scope::FileMetadata,
            Encryption::Unencrypted,
            Encryption::Unknown,
        ),
    ] {
        reject(
            Scene::EncryptedLeaf,
            Witness {
                target: Target::Leaf,
                scope,
                captured,
                forged,
                use_set: UseSet::UnusedReference,
            },
        )?;
    }
    Ok(())
}

#[test]
fn standalone_and_whole_archive_file_use_descriptions_stay_unknown_in_every_domain() -> TestResult {
    for target in [Target::Standalone, Target::WholeArchive] {
        for scope in [
            Scope::Payload,
            Scope::ArchiveDirectoryMetadata,
            Scope::FileMetadata,
        ] {
            for forged in [Encryption::Unencrypted, Encryption::Encrypted] {
                reject(
                    Scene::Plain,
                    Witness {
                        target,
                        scope,
                        captured: Encryption::Unknown,
                        forged,
                        use_set: UseSet::UnusedReference,
                    },
                )?;
            }
        }
    }
    Ok(())
}

#[test]
fn unknown_kind_with_an_encrypted_outer_ancestor_keeps_its_known_witness() -> TestResult {
    for scope in [
        Scope::Payload,
        Scope::ArchiveDirectoryMetadata,
        Scope::FileMetadata,
    ] {
        for forged in [Encryption::Unencrypted, Encryption::Unknown] {
            reject(
                Scene::EncryptedOuterUnknownAncestor,
                Witness {
                    target: Target::UnknownAncestor,
                    scope,
                    captured: Encryption::Encrypted,
                    forged,
                    use_set: UseSet::UnusedReference,
                },
            )?;
        }
    }
    Ok(())
}

#[test]
fn genuine_encrypted_leaf_metadata_remains_plain_and_ancestor_metadata_is_distinct() -> TestResult {
    for (scene, payload, metadata) in [
        (
            Scene::Plain,
            Encryption::Unencrypted,
            Encryption::Unencrypted,
        ),
        (
            Scene::EncryptedLeaf,
            Encryption::Encrypted,
            Encryption::Unencrypted,
        ),
        (
            Scene::EncryptedAncestor,
            Encryption::Encrypted,
            Encryption::Encrypted,
        ),
        (
            Scene::UnknownAncestor,
            Encryption::Unknown,
            Encryption::Unknown,
        ),
        (
            Scene::EncryptedOuterUnknownAncestor,
            Encryption::Encrypted,
            Encryption::Encrypted,
        ),
    ] {
        let fixture = Fixture::new(scene)?;
        let snapshot = Snapshot::load(&fixture)?;
        assert_domain(&snapshot, Scope::Payload, payload)?;
        assert_domain(&snapshot, Scope::ArchiveDirectoryMetadata, metadata)?;
        assert_domain(&snapshot, Scope::FileMetadata, metadata)?;
        assert!(
            fixture.reuse()?.is_some(),
            "{scene:?}: original producer generation is reusable"
        );
    }
    Ok(())
}

#[test]
fn genuine_unknown_inputs_and_unknown_ancestors_keep_both_valid_metadata_states() -> TestResult {
    let fixture = Fixture::new(Scene::UnknownAncestor)?;
    let snapshot = Snapshot::load(&fixture)?;
    let mut plain = false;
    let mut unknown = false;
    for source in &snapshot.sources {
        if source.get("kind").and_then(serde_json::Value::as_str) != Some("unknown") {
            continue;
        }
        let id = source.get("id").ok_or("actual unknown source id")?;
        for used in &snapshot.uses {
            if used.get("source") != Some(id)
                || used.get("scope").and_then(serde_json::Value::as_str)
                    != Some("archiveDirectoryMetadata")
            {
                continue;
            }
            let state: Encryption = serde_json::from_value(
                used.get("effectiveEncryption")
                    .ok_or("actual captured metadata")?
                    .clone(),
            )?;
            plain |= state == Encryption::Unencrypted;
            unknown |= state == Encryption::Unknown;
        }
    }
    assert!(
        plain && unknown,
        "genuine unknown-kind metadata permits both plain and unknown"
    );
    assert!(
        fixture.reuse()?.is_some(),
        "the physical consistency check preserves original ambiguity"
    );
    Ok(())
}

fn reject(scene: Scene, witness: Witness) -> TestResult {
    let mut fixture = Fixture::new(scene)?;
    assert!(
        fixture.reuse()?.is_some(),
        "{scene:?}: genuine producer positive control"
    );
    let mut snapshot = Snapshot::load(&fixture)?;
    snapshot.forge(witness)?;
    snapshot.publish_forgery(&mut fixture)?;
    let result = fixture.reuse();
    assert!(
        matches!(&result, Err(MediaError::Destination(detail)) if detail == CONTRADICTION),
        "{scene:?}/{:?}/{:?}: fully rebound metadata must fail physical consistency, got {result:?}",
        witness.target,
        witness.scope
    );
    Ok(())
}

fn assert_domain(snapshot: &Snapshot, scope: Scope, expected: Encryption) -> TestResult {
    let source = snapshot
        .source(Target::Leaf)?
        .get("id")
        .ok_or("physical leaf id")?;
    let mut observed = false;
    for used in &snapshot.uses {
        if used.get("source") != Some(source) {
            continue;
        }
        let actual: Scope =
            serde_json::from_value(used.get("scope").ok_or("actual domain")?.clone())?;
        if actual != scope {
            continue;
        }
        let state: Encryption = serde_json::from_value(
            used.get("effectiveEncryption")
                .ok_or("actual captured state")?
                .clone(),
        )?;
        assert_eq!(
            state, expected,
            "the genuine physical source keeps its captured {scope:?} domain"
        );
        observed = true;
    }
    assert!(observed, "the original producer observed {scope:?}");
    Ok(())
}
#[path = "origin_physical_consistency_tests.rs"]
mod consistency;
#[path = "origin_physical_consistency_mutation.rs"]
mod consistency_mutation;

#[path = "origin_partial_declaration_tests.rs"]
mod partial_declarations;
