// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Actual containing-context evidence, unsupported declarations and late encrypted witnesses.

use p4kx_extraction::{
    InputScope, SourceCoordinateOmission, SourceDetails, SourceInput, SourceUseRole, SourceUses,
    SourceValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::source_minimal_proof as support;
use crate::source_minimal_proof::TestResult;

#[test]
fn original_unknown_view_identity_stays_stable_through_full_upgrade_without_aliasing_other_views()
-> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("room.pak", inner)))?;
    let detached = support::first(&root)?.clone();
    let one = root.open_container(&detached)?;
    let two = root.open_container(&detached)?;
    let sources = SourceUses::new();
    let sparse = support::source(&sources, &one, SourceDetails::CaptureLimited)?;
    let again = support::source(&sources, &one, SourceDetails::CaptureLimited)?;
    assert_eq!(
        sparse.source_ref(),
        again.source_ref(),
        "same original unproved view is stable across borrowed observations"
    );
    let other = support::source(&sources, &two, SourceDetails::CaptureLimited)?;
    assert_ne!(
        sparse.source_ref(),
        other.source_ref(),
        "different unproved views cannot alias through equal root/leaf ordinals"
    );
    assert!(
        sparse.matches_entry(
            &one.observed_entry(support::first(&one)?)
                .ok_or("same original view")?
        )?,
        "same unknown context retains its genuine runtime owner"
    );
    assert!(
        !sparse.matches_entry(
            &two.observed_entry(support::first(&two)?)
                .ok_or("different unknown view")?
        )?,
        "unknown ancestor coordinates do not fabricate cross-view identity"
    );
    let origin = one
        .entry_origin(support::first(&one)?)
        .ok_or("original full snapshot")?;
    let full = sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?;
    assert_eq!(
        sparse.source_ref(),
        full.source_ref(),
        "a genuine full snapshot upgrades an explicitly sparse unknown view under the same source ID"
    );
    checks::upgraded_unknown_source(&sources, &sparse)?;
    checks::incomplete_unknown_upgrade(&sources, &full, &one)?;
    Ok(())
}

#[test]
fn nested_minimal_proofs_preserve_original_prefix_domains_without_payload_key_health_changes()
-> TestResult {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.ext", b"leaf"))
        .build();
    let middle = ArchiveBuilder::new()
        .entry(TestEntry::deflated("inner.pak", leaf).encrypted())
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("outer.pak", middle)))?;
    let outer = root.open_container(support::first(&root)?)?;
    let inner = outer.open_container(support::first(&outer)?)?;
    let before = root.key_demand().snapshot();
    let sources = SourceUses::new();
    let leaf_scope = support::source(&sources, &inner, SourceDetails::CaptureLimited)?;
    let outer_scope = leaf_scope.ancestor(0)?.ok_or("outer original source")?;
    let inner_scope = leaf_scope.ancestor(1)?.ok_or("inner original source")?;
    checks::prefix_domains(&sources, &outer_scope, &inner_scope, &leaf_scope)?;
    assert!(
        outer_scope.matches_entry(
            &root
                .observed_entry(support::first(&root)?)
                .ok_or("outer actual view")?
        )?,
        "ancestor scope carries the original ancestor binding"
    );
    assert!(
        inner_scope.matches_entry(
            &outer
                .observed_entry(support::first(&outer)?)
                .ok_or("inner actual view")?
        )?,
        "inner scope preserves its own genuine containing-view identity"
    );
    let after = root.key_demand().snapshot();
    assert_eq!(
        before.unread(),
        after.unread(),
        "metadata retention does not change actual archive-key health"
    );
    checks::prefix_rows(&sources)?;
    Ok(())
}

#[test]
fn minimal_proofs_use_conservative_original_declarations_instead_of_boolean_read_flags()
-> TestResult {
    for (word, marker, stored) in [
        (0, Some(false), "unencrypted"),
        (1, Some(true), "encrypted"),
        (7, None, "unknown"),
    ] {
        for v2 in [false, true] {
            let builder = if v2 {
                ArchiveBuilder::v2()
            } else {
                ArchiveBuilder::new()
            };
            let (_directory, archive) = support::open(
                &builder
                    .entry(TestEntry::stored("source.bin", b"same").with_encryption_field(word)),
            )?;
            let sources = SourceUses::new();
            let scope = support::source(&sources, &archive, SourceDetails::CaptureLimited)?;
            let value = sources.value(SourceValue::Present(()), &[support::used(&scope)?], true);
            assert_eq!(
                value.origin().contains_encrypted_source(),
                marker,
                "stored word {word} in v2={v2} keeps its original conservative meaning"
            );
            support::assert_closed(&sources.freeze()?)?;
            let wire = serde_json::to_value(sources.freeze()?)?;
            assert_eq!(
                wire["sources"][0]["storedEncryption"], stored,
                "minimal proof never replaces unsupported declarations with a boolean reader flag"
            );
        }
    }
    Ok(())
}

#[test]
fn late_known_encrypted_opaque_contributor_stays_referenced_after_many_plain_uses() -> TestResult {
    const PRIOR_PLAIN_USES: u64 = 4096;
    let (_directory, archive) = support::open(
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("plain.bin", b"same"))
            .entry(TestEntry::stored("secret.bin", b"same").encrypted()),
    )?;
    let sources = SourceUses::new();
    let plain = support::source(&sources, &archive, SourceDetails::CaptureLimited)?;
    let encrypted = checks::encrypted_second_source(&sources, &archive)?;
    let mut uses = checks::plain_selector_uses(&plain, PRIOR_PLAIN_USES)?;
    let witness = encrypted.at_opaque(
        SourceUseRole::Conversion,
        SourceCoordinateOmission::CaptureLimit,
    )?;
    uses.push(witness.clone());
    let value = sources.value(SourceValue::Present(()), &uses, true);
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(true),
        "an encrypted actual input admitted after many plain uses remains dominant"
    );
    assert!(
        !value.origin().source_set_complete(),
        "the actual omitted encrypted coordinate forces honest incompleteness"
    );
    assert!(
        value
            .origin()
            .source_uses_refs()
            .contains(witness.reference()),
        "true is backed by a directly retained original encrypted witness"
    );
    let before = serde_json::to_vec(&sources.freeze()?)?;
    let repeated = encrypted.at_opaque(
        SourceUseRole::Conversion,
        SourceCoordinateOmission::CaptureLimit,
    )?;
    assert_eq!(
        witness.reference(),
        repeated.reference(),
        "identical original site deduplicates before any optional capture policy"
    );
    assert_eq!(
        before,
        serde_json::to_vec(&sources.freeze()?)?,
        "repeat opaque registration allocates no extra source or use row"
    );
    Ok(())
}

#[test]
fn full_legacy_unknown_view_is_not_duplicated_or_downgraded_by_limited_capture() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("leaf.bin", b"same"))
        .build();
    let (_directory, root) =
        support::open(&ArchiveBuilder::new().entry(TestEntry::stored("room.pak", inner)))?;
    let detached = support::first(&root)?.clone();
    let view = root.open_container(&detached)?;
    let original = view
        .entry_origin(support::first(&view)?)
        .ok_or("full original source")?;
    let sources = SourceUses::new();
    let full = sources.observe(SourceInput::Archive(&original), InputScope::Payload)?;
    let full_bytes = serde_json::to_vec(&sources.freeze()?)?;
    let limited = support::source(&sources, &view, SourceDetails::CaptureLimited)?;
    assert_eq!(
        full.source_ref(),
        limited.source_ref(),
        "new capture API reuses the original recorded numeric nonce rather than fabricating a second physical source"
    );
    assert_eq!(
        full_bytes,
        serde_json::to_vec(&sources.freeze()?)?,
        "capture cannot downgrade or duplicate previously complete directory details"
    );
    let repeated = sources.observe(SourceInput::Archive(&original), InputScope::Payload)?;
    assert_eq!(
        repeated.source_ref(),
        full.source_ref(),
        "explicit detail transition retains the selected original runtime context"
    );
    Ok(())
}

#[path = "source_minimal_context_checks.rs"]
mod checks;
