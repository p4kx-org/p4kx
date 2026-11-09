// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

#[path = "origin_partial_declaration_controls.rs"]
mod controls;
#[path = "origin_partial_declaration_support.rs"]
mod support;
use support::{
    DECLARATION, Encryption, Fixture, Scene, TestResult, changes, checked, leaf, reject,
    unavailable_occurrence, unknown, unreadable,
};

#[test]
fn encrypted_declaration_survives_an_unavailable_sibling_occurrence() -> TestResult {
    let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
    let mut snapshot = checked(&fixture)?;
    unavailable_occurrence(&mut snapshot)?;
    changes::relabel_stored(&mut snapshot, Encryption::Unencrypted)?;
    reject(&mut fixture, &mut snapshot)
}

#[test]
fn plain_declaration_survives_an_unavailable_sibling_occurrence() -> TestResult {
    let mut fixture = Fixture::new(Scene::Plain)?;
    let mut snapshot = checked(&fixture)?;
    unavailable_occurrence(&mut snapshot)?;
    changes::relabel_stored(&mut snapshot, Encryption::Encrypted)?;
    reject(&mut fixture, &mut snapshot)
}

#[test]
fn concrete_unknown_declaration_survives_an_unavailable_sibling_occurrence() -> TestResult {
    for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
        let mut fixture = unknown(None)?;
        let mut snapshot = checked(&fixture)?;
        unavailable_occurrence(&mut snapshot)?;
        changes::relabel_stored(&mut snapshot, forged)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn unsupported_word_remains_unknown_when_the_block_length_is_unavailable() -> TestResult {
    for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
        let mut fixture = unknown(None)?;
        let mut snapshot = checked(&fixture)?;
        unreadable(&mut snapshot, "declaredBlockLength")?;
        changes::relabel_stored(&mut snapshot, forged)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn invalid_block_shape_remains_unknown_when_word_and_bytes_are_unavailable() -> TestResult {
    for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
        let mut fixture = unknown(Some(Vec::new()))?;
        let mut snapshot = checked(&fixture)?;
        unreadable(&mut snapshot, "rawWord")?;
        unreadable(&mut snapshot, "rawBytes")?;
        changes::relabel_stored(&mut snapshot, forged)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn unsupported_kind_remains_unknown_when_all_optional_inner_facts_are_unavailable() -> TestResult {
    for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
        let mut fixture = unknown(None)?;
        let mut snapshot = checked(&fixture)?;
        changes::replace(
            &mut snapshot,
            &format!("{DECLARATION}/kind"),
            json!("unknown"),
        )?;
        for field in ["rawWord", "rawBytes", "declaredBlockLength"] {
            unreadable(&mut snapshot, field)?;
        }
        changes::relabel_stored(&mut snapshot, forged)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn known_short_bytes_remain_unknown_when_the_block_length_is_unavailable() -> TestResult {
    for bytes in [Vec::new(), vec![1]] {
        for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
            let mut fixture = unknown(Some(bytes.clone()))?;
            let mut snapshot = checked(&fixture)?;
            unreadable(&mut snapshot, "declaredBlockLength")?;
            changes::relabel_stored(&mut snapshot, forged)?;
            reject(&mut fixture, &mut snapshot)?;
        }
    }
    Ok(())
}

#[test]
fn known_words_exclude_the_opposite_state_with_unavailable_block_length() -> TestResult {
    for (scene, forged) in [
        (Scene::EncryptedLeaf, Encryption::Unencrypted),
        (Scene::Plain, Encryption::Encrypted),
    ] {
        let mut fixture = Fixture::new(scene)?;
        let mut snapshot = checked(&fixture)?;
        unreadable(&mut snapshot, "declaredBlockLength")?;
        changes::relabel_stored(&mut snapshot, forged)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn known_plain_word_cannot_become_encrypted_when_zip_flags_are_unavailable() -> TestResult {
    let mut fixture = Fixture::new(Scene::Plain)?;
    let mut snapshot = checked(&fixture)?;
    support::unavailable(&mut snapshot, "/zipFlags")?;
    changes::relabel_stored(&mut snapshot, Encryption::Encrypted)?;
    reject(&mut fixture, &mut snapshot)
}

#[test]
fn conflicting_available_occurrences_remain_unknown_with_an_unavailable_third() -> TestResult {
    for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
        let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
        let mut snapshot = checked(&fixture)?;
        let declarations = leaf(&mut snapshot)?
            .pointer_mut("/declarations/value")
            .and_then(Value::as_array_mut)
            .ok_or("actual declarations")?;
        let mut plain = declarations
            .first()
            .ok_or("original encrypted declaration")?
            .clone();
        *plain
            .pointer_mut("/value/rawWord/value")
            .ok_or("available word")? = json!(0);
        *plain
            .pointer_mut("/value/rawBytes/value")
            .ok_or("available bytes")? = json!([0, 0]);
        declarations.push(plain);
        unavailable_occurrence(&mut snapshot)?;
        changes::relabel_stored(&mut snapshot, forged)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn conflicting_word_and_bytes_are_not_erased_by_unavailable_other_facts() -> TestResult {
    let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
    let mut snapshot = checked(&fixture)?;
    unreadable(&mut snapshot, "declaredBlockLength")?;
    changes::replace(
        &mut snapshot,
        &format!("{DECLARATION}/rawBytes/value"),
        json!([0, 0]),
    )?;
    snapshot.publish_forgery(&mut fixture)?;
    let result = fixture.reuse();
    assert!(
        matches!(&result, Err(crate::MediaError::Destination(detail)) if detail == "raw declaration word disagrees with available raw bytes"),
        "contradictory present raw facts retain the exact existing error: {result:?}"
    );
    Ok(())
}

#[path = "origin_fact_state_tests.rs"]
mod fact_states;
