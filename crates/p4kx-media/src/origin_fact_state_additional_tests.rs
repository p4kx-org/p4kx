// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::support::{
    DECLARATION, Encryption, Fixture, Scene, TestResult, changes, checked, fact_state, reject,
    unavailable,
};
use super::KNOWN_NON_PRESENT;

#[test]
fn a_genuine_v2_absent_length_cannot_be_replaced_by_a_malformed_length() -> TestResult {
    for encrypted in [false, true] {
        for state in ["null", "invalid", "unsupported"] {
            let entry = if encrypted {
                TestEntry::zstd("leaf.ext", b"original synthetic bytes").encrypted()
            } else {
                TestEntry::stored("leaf.ext", b"original synthetic bytes")
            };
            let mut fixture = Fixture::from_builder(&ArchiveBuilder::v2().entry(entry))?;
            let mut snapshot = checked(&fixture)?;
            fact_state(
                &mut snapshot,
                &format!("{DECLARATION}/declaredBlockLength"),
                state,
            )?;
            reject(&mut fixture, &mut snapshot)?;
        }
    }
    Ok(())
}

#[test]
fn known_non_present_bytes_cannot_become_unreadable_or_agree_with_a_complete_word() -> TestResult {
    for state in KNOWN_NON_PRESENT {
        for forged in [Encryption::Encrypted, Encryption::Unencrypted] {
            let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
            let mut snapshot = checked(&fixture)?;
            unavailable(&mut snapshot, &format!("{DECLARATION}/rawWord"))?;
            unavailable(&mut snapshot, &format!("{DECLARATION}/declaredBlockLength"))?;
            fact_state(&mut snapshot, &format!("{DECLARATION}/rawBytes"), state)?;
            changes::relabel_stored(&mut snapshot, forged)?;
            reject(&mut fixture, &mut snapshot)?;
        }
        let mut fixture = Fixture::new(Scene::EncryptedLeaf)?;
        let mut snapshot = checked(&fixture)?;
        fact_state(&mut snapshot, &format!("{DECLARATION}/rawBytes"), state)?;
        snapshot.publish_forgery(&mut fixture)?;
        let result = fixture.reuse();
        assert!(
            matches!(&result, Err(crate::MediaError::Destination(detail)) if detail == "raw declaration word disagrees with available raw bytes"),
            "a known missing/malformed byte field contradicts an available complete word: {result:?}"
        );
    }
    Ok(())
}
