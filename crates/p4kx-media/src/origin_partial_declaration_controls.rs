// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::support::{
    Encryption, Fixture, Scene, TestResult, accept, changes, checked, unavailable_occurrence,
    unknown, unreadable,
};

#[test]
fn missing_occurrences_keep_matching_original_known_and_unknown_states() -> TestResult {
    for scene in [Scene::Plain, Scene::EncryptedLeaf] {
        let mut fixture = Fixture::new(scene)?;
        let mut snapshot = checked(&fixture)?;
        unavailable_occurrence(&mut snapshot)?;
        accept(&mut fixture, &mut snapshot)?;
    }
    let mut fixture = unknown(None)?;
    let mut snapshot = checked(&fixture)?;
    unavailable_occurrence(&mut snapshot)?;
    accept(&mut fixture, &mut snapshot)
}

#[test]
fn an_unavailable_occurrence_may_legitimately_make_known_consensus_unknown() -> TestResult {
    for scene in [Scene::Plain, Scene::EncryptedLeaf] {
        let mut fixture = Fixture::new(scene)?;
        let mut snapshot = checked(&fixture)?;
        unavailable_occurrence(&mut snapshot)?;
        changes::relabel_stored(&mut snapshot, Encryption::Unknown)?;
        accept(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn known_words_with_unavailable_shape_allow_matching_or_unknown_original_states() -> TestResult {
    for (scene, matching) in [
        (Scene::Plain, Encryption::Unencrypted),
        (Scene::EncryptedLeaf, Encryption::Encrypted),
    ] {
        for state in [matching, Encryption::Unknown] {
            let mut fixture = Fixture::new(scene)?;
            let mut snapshot = checked(&fixture)?;
            unreadable(&mut snapshot, "declaredBlockLength")?;
            changes::relabel_stored(&mut snapshot, state)?;
            accept(&mut fixture, &mut snapshot)?;
        }
    }
    Ok(())
}

#[test]
fn all_unavailable_inner_facts_do_not_discard_matching_minimal_proof() -> TestResult {
    for scene in [Scene::Plain, Scene::EncryptedLeaf] {
        let mut fixture = Fixture::new(scene)?;
        let mut snapshot = checked(&fixture)?;
        for field in ["rawWord", "rawBytes", "declaredBlockLength"] {
            unreadable(&mut snapshot, field)?;
        }
        accept(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn unavailable_zip_flags_leave_plain_and_unknown_possible_without_inventing_encryption()
-> TestResult {
    for state in [Encryption::Unencrypted, Encryption::Unknown] {
        let mut fixture = Fixture::new(Scene::Plain)?;
        let mut snapshot = checked(&fixture)?;
        super::support::unavailable(&mut snapshot, "/zipFlags")?;
        changes::relabel_stored(&mut snapshot, state)?;
        accept(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}
