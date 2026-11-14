// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::support::{
    DECLARATION, Encryption, Fixture, Scene, TestResult, accept, changes, checked, fact_state,
    leaf, reject, unavailable, unavailable_occurrence, unknown,
};

const KNOWN_NON_PRESENT: &[&str] = &["absent", "null", "invalid", "unsupported"];

#[test]
fn genuine_invalid_word_cannot_become_arbitrary_when_other_details_are_unreadable() -> TestResult {
    for bytes in [Vec::new(), vec![1]] {
        for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
            let mut fixture = unknown(Some(bytes.clone()))?;
            let mut snapshot = checked(&fixture)?;
            assert_eq!(
                leaf(&mut snapshot)?
                    .pointer(&format!("{DECLARATION}/rawWord/state"))
                    .and_then(Value::as_str),
                Some("invalid"),
                "the owning producer marks its short word Invalid"
            );
            unavailable(&mut snapshot, &format!("{DECLARATION}/rawBytes"))?;
            unavailable(&mut snapshot, &format!("{DECLARATION}/declaredBlockLength"))?;
            changes::relabel_stored(&mut snapshot, forged)?;
            reject(&mut fixture, &mut snapshot)?;
        }
    }
    Ok(())
}

#[test]
fn known_non_present_words_are_not_unreadable_words() -> TestResult {
    for state in KNOWN_NON_PRESENT {
        for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
            let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
            let mut snapshot = checked(&fixture)?;
            fact_state(&mut snapshot, &format!("{DECLARATION}/rawWord"), state)?;
            unavailable(&mut snapshot, &format!("{DECLARATION}/rawBytes"))?;
            unavailable(&mut snapshot, &format!("{DECLARATION}/declaredBlockLength"))?;
            changes::relabel_stored(&mut snapshot, forged)?;
            reject(&mut fixture, &mut snapshot)?;
        }
    }
    Ok(())
}

#[test]
fn known_non_present_cig_lengths_are_not_unavailable_length_evidence() -> TestResult {
    for state in KNOWN_NON_PRESENT {
        let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
        let mut snapshot = checked(&fixture)?;
        fact_state(
            &mut snapshot,
            &format!("{DECLARATION}/declaredBlockLength"),
            state,
        )?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn absent_or_malformed_zip_flags_do_not_invent_a_plain_zip_header() -> TestResult {
    for state in KNOWN_NON_PRESENT {
        let mut fixture = Fixture::new(Scene::Plain)?;
        let mut snapshot = checked(&fixture)?;
        fact_state(&mut snapshot, "/zipFlags", state)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn known_non_present_declaration_lists_cannot_stand_for_unknown_present_declarations() -> TestResult
{
    for scene in [Scene::Plain, Scene::EncryptedLeaf] {
        for state in KNOWN_NON_PRESENT {
            let mut fixture = Fixture::new(scene)?;
            let mut snapshot = checked(&fixture)?;
            fact_state(&mut snapshot, "/declarations", state)?;
            reject(&mut fixture, &mut snapshot)?;
        }
    }
    Ok(())
}

#[test]
fn an_absent_occurrence_cannot_invent_a_conflicting_declaration() -> TestResult {
    for scene in [Scene::Plain, Scene::EncryptedLeaf] {
        let mut fixture = Fixture::new(scene)?;
        let mut snapshot = checked(&fixture)?;
        unavailable_occurrence(&mut snapshot)?;
        fact_state(&mut snapshot, "/declarations/value/1", "absent")?;
        changes::relabel_stored(&mut snapshot, Encryption::Unknown)?;
        reject(&mut fixture, &mut snapshot)?;
    }
    Ok(())
}

#[test]
fn a_known_malformed_occurrence_cannot_disappear_as_unavailable_detail() -> TestResult {
    for scene in [Scene::Plain, Scene::EncryptedLeaf] {
        for state in ["null", "invalid", "unsupported"] {
            let mut fixture = Fixture::new(scene)?;
            let mut snapshot = checked(&fixture)?;
            unavailable_occurrence(&mut snapshot)?;
            fact_state(&mut snapshot, "/declarations/value/1", state)?;
            reject(&mut fixture, &mut snapshot)?;
        }
    }
    Ok(())
}

#[test]
fn available_complete_bytes_cannot_agree_with_a_known_non_word_state() -> TestResult {
    for state in KNOWN_NON_PRESENT {
        let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
        let mut snapshot = checked(&fixture)?;
        fact_state(&mut snapshot, &format!("{DECLARATION}/rawWord"), state)?;
        snapshot.publish_forgery(&mut fixture)?;
        let result = fixture.reuse();
        assert!(
            matches!(&result, Err(crate::MediaError::Destination(detail)) if detail == "raw declaration word disagrees with available raw bytes"),
            "known negative word evidence contradicts available complete bytes: {result:?}"
        );
    }
    Ok(())
}

#[test]
fn actual_v2_absent_length_and_zip_flags_remain_canonical_known_absence() -> TestResult {
    for entry in [
        TestEntry::stored("leaf.ext", b"original synthetic bytes"),
        TestEntry::zstd("leaf.ext", b"original synthetic bytes").encrypted(),
    ] {
        let fixture = Fixture::from_builder(&ArchiveBuilder::v2().entry(entry))?;
        let mut snapshot = checked(&fixture)?;
        for pointer in [
            "/zipFlags/state",
            "/declarations/value/0/value/declaredBlockLength/state",
        ] {
            assert_eq!(
                leaf(&mut snapshot)?
                    .pointer(pointer)
                    .and_then(Value::as_str),
                Some("absent"),
                "the genuine v2 producer declares no such field"
            );
        }
        assert!(fixture.reuse()?.is_some());
    }
    Ok(())
}

#[test]
fn genuinely_unreadable_detail_preserves_matching_proof_while_known_bad_detail_requires_unknown()
-> TestResult {
    for scene in [Scene::Plain, Scene::EncryptedLeaf] {
        let mut fixture = Fixture::new(scene)?;
        let mut snapshot = checked(&fixture)?;
        unavailable(&mut snapshot, "/declarations")?;
        accept(&mut fixture, &mut snapshot)?;
    }
    let mut fixture = unknown(Some(vec![1]))?;
    let mut snapshot = checked(&fixture)?;
    unavailable(&mut snapshot, &format!("{DECLARATION}/rawBytes"))?;
    unavailable(&mut snapshot, &format!("{DECLARATION}/declaredBlockLength"))?;
    accept(&mut fixture, &mut snapshot)
}

#[path = "origin_fact_state_additional_tests.rs"]
mod additional;
