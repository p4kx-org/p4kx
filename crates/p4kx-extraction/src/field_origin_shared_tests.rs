// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;
use std::sync::Arc;

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::{
    Archive, InputScope, SourceCoordinateOmission, SourceEvidenceError, SourceInput,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

use super::FieldOrigin;
use super::refs::OriginRefs;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn empty_origin_fields_reuse_payloads_with_distinct_source_authority() -> TestResult {
    let sources = SourceUses::new();
    let other = SourceUses::new();
    let shared = sources.shared_origin(&[], true);
    for complete in [true, false] {
        let authored = sources.origin(&[], complete);
        let reused = sources.origin_with_shared(&shared, &[], complete)?;
        let foreign = other.origin(&[], complete);
        let marker = complete.then_some(false);
        let decoded = FieldOrigin::from_wire(marker, Vec::new(), complete)?;
        for origin in [&reused, &foreign, &decoded] {
            assert!(
                Arc::ptr_eq(&authored.fields, &origin.fields),
                "the exact empty data is shared independently of the source inventory"
            );
        }
        let original_identity = authored
            .identity
            .as_ref()
            .ok_or("original source authority")?;
        assert!(
            reused.belongs_to(original_identity),
            "locally shared origin retains authority"
        );
        assert!(
            !foreign.belongs_to(original_identity),
            "shared fields grant no foreign authority"
        );
        assert!(
            !decoded.belongs_to(original_identity),
            "wire data grants no original authority"
        );
        assert_eq!(
            serde_json::to_value(&authored)?,
            serde_json::json!({
                "containsEncryptedSource": marker,
                "sourceUsesRefs": [],
                "sourceSetComplete": complete
            }),
            "the two empty markers retain their exact wire shape"
        );
    }
    Ok(())
}

#[test]
fn one_distinct_contributor_stays_inline_with_the_same_wire() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let used = evidence(&scope, "value", SourceUseRole::Payload)?;
    let attempt = evidence(&scope, "attempt", SourceUseRole::Attempted)?;
    let local = sources.origin(&[attempt, used.clone(), used.clone()], true);
    let shared = sources.shared_origin(std::slice::from_ref(&used), true);
    let basis = sources.origin_with_shared(&shared, &[], true)?;
    let decoded = FieldOrigin::from_wire(
        None,
        vec![crate::SourceUseRef::decoded(
            used.reference().as_str().into(),
        )],
        true,
    )?;
    for origin in [&local, &decoded] {
        assert!(
            matches!(&origin.fields.source_uses_refs, OriginRefs::Single(reference)
                if reference == used.reference()),
            "one distinct contribution uses no reference vector"
        );
        assert_eq!(
            origin.source_uses_refs(),
            std::slice::from_ref(used.reference()),
            "the borrowed contribution stays the exact original reference"
        );
        assert_eq!(
            origin, &basis,
            "ownership storage leaves equality unchanged"
        );
        assert_eq!(
            serde_json::to_vec(origin)?,
            serde_json::to_vec(&basis)?,
            "the same contribution set has the same wire representation"
        );
    }
    Ok(())
}

#[test]
fn unchanged_shared_origins_reuse_the_same_field_bundle() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let used = evidence(&scope, "basis", SourceUseRole::Selector)?;
    let shared = sources.shared_origin(std::slice::from_ref(&used), true);
    let first = sources.origin_with_shared(&shared, &[], true)?;
    let repeated = sources.origin_with_shared(&shared, &[used], true)?;
    let incomplete = sources.origin_with_shared(&shared, &[], false)?;
    assert!(
        Arc::ptr_eq(&first.fields, &repeated.fields),
        "an identical contributor set and certainty share the bundle"
    );
    assert!(
        !Arc::ptr_eq(&first.fields, &incomplete.fields),
        "a changed certainty requires its own bundle"
    );
    assert!(
        first.source_set_complete(),
        "the original proof is complete"
    );
    assert!(
        !incomplete.source_set_complete(),
        "an incomplete projection keeps its own certainty"
    );
    assert_eq!(
        first.source_uses_refs(),
        incomplete.source_uses_refs(),
        "uncertainty leaves the actual input references intact"
    );
    Ok(())
}

#[test]
fn a_shared_origin_serializes_the_sorted_unique_union_without_expanding_it() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let first = evidence(&scope, "first", SourceUseRole::Payload)?;
    let middle = evidence(&scope, "middle", SourceUseRole::Conversion)?;
    let last = evidence(&scope, "last", SourceUseRole::Selector)?;
    let skipped = evidence(&scope, "skipped", SourceUseRole::Attempted)?;
    let untouched = evidence(&scope, "reference", SourceUseRole::Reference)?;
    let common = [middle.clone(), middle.clone(), first.clone(), skipped];
    let local = [last.clone(), untouched, first.clone(), last.clone()];
    let shared = sources.shared_origin(&common, true);
    let joined =
        sources.value_with_shared_origin(SourceValue::Present(17_u8), &shared, &local, true)?;
    let flat = sources.value(
        SourceValue::Present(17_u8),
        &[first.clone(), middle.clone(), last.clone()],
        true,
    );
    assert_eq!(
        serde_json::to_vec(&joined)?,
        serde_json::to_vec(&flat)?,
        "shared and flat storage have identical sourced-value bytes"
    );
    assert_eq!(
        joined, flat,
        "equality compares the complete logical contribution sequence"
    );
    assert!(
        joined
            .origin()
            .fields
            .source_uses_refs
            .is_shared_unexpanded(),
        "serialization and equality leave the shared references unexpanded"
    );
    let wire = serde_json::to_value(&joined)?;
    let schema = serde_json::to_value(schemars::schema_for!(SourcedValue<u8>))?;
    assert!(
        jsonschema::validator_for(&schema)?.is_valid(&wire),
        "the existing sourced-value schema accepts the same wire"
    );
    let mut expected = [
        first.reference().clone(),
        middle.reference().clone(),
        last.reference().clone(),
    ];
    expected.sort();
    assert_eq!(
        joined.origin().source_uses_refs(),
        expected,
        "the borrowed slice remains sorted and unique"
    );
    assert!(
        !joined
            .origin()
            .fields
            .source_uses_refs
            .is_shared_unexpanded(),
        "requesting the borrowed slice expands it on demand"
    );
    Ok(())
}

#[test]
fn combining_origins_leaves_a_shared_input_unexpanded() -> TestResult {
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let first = evidence(&scope, "basis", SourceUseRole::Selector)?;
    let second = evidence(&scope, "extension", SourceUseRole::Payload)?;
    let third = evidence(&scope, "aggregate", SourceUseRole::Conversion)?;
    let basis = sources.shared_origin(std::slice::from_ref(&first), true);
    let shared = sources.origin_with_shared(&basis, std::slice::from_ref(&second), true)?;
    let extra = sources.origin(std::slice::from_ref(&third), false);

    let combined = sources.origin_with_origins([&shared, &extra], true)?;
    let flat = sources.origin(&[first, second, third], false);
    assert_eq!(
        serde_json::to_vec(&combined)?,
        serde_json::to_vec(&flat)?,
        "aggregation retains the same references and completeness on the wire"
    );
    assert!(
        shared.fields.source_uses_refs.is_shared_unexpanded(),
        "aggregation must leave the shared input's slice cache empty"
    );
    Ok(())
}

#[test]
fn shared_origin_retains_encryption_opaque_and_incomplete_rules() -> TestResult {
    let folder = tempfile::tempdir()?;
    let sources = SourceUses::new();
    let clear = archive_scope(folder.path(), &sources, false)?;
    let encrypted = archive_scope(folder.path(), &sources, true)?;
    let clear_use = evidence(&clear, "clear", SourceUseRole::Selector)?;
    let encrypted_use = evidence(&encrypted, "encrypted", SourceUseRole::Payload)?;
    let attempt = evidence(&encrypted, "failed", SourceUseRole::Attempted)?;
    let unknown = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let unknown_use = evidence(&unknown, "standalone", SourceUseRole::Selector)?;
    let opaque = sources.opaque_use(&encrypted_use, SourceCoordinateOmission::Unavailable)?;
    let hidden_clear = sources.opaque_use(&clear_use, SourceCoordinateOmission::Unavailable)?;
    for (base, local, complete, marker, expected_complete) in [
        (clear_use.clone(), attempt, true, Some(false), true),
        (
            clear_use.clone(),
            encrypted_use.clone(),
            true,
            Some(true),
            true,
        ),
        (clear_use.clone(), unknown_use, true, None, true),
        (clear_use.clone(), clear_use.clone(), false, None, false),
        (clear_use.clone(), opaque, true, Some(true), false),
        (hidden_clear, clear_use, true, None, false),
    ] {
        let shared = sources.shared_origin(std::slice::from_ref(&base), true);
        let joined = sources.value_with_shared_origin(
            SourceValue::Absent::<u8>,
            &shared,
            std::slice::from_ref(&local),
            complete,
        )?;
        let flat = sources.value(SourceValue::Absent::<u8>, &[base, local], complete);
        assert_eq!(
            joined, flat,
            "shared and flat origins apply the same evidence rules"
        );
        assert_eq!(
            joined.origin().contains_encrypted_source(),
            marker,
            "the expected encryption marker"
        );
        assert_eq!(
            joined.origin().source_set_complete(),
            expected_complete,
            "the expected source completeness"
        );
    }
    check_incomplete_basis(&sources, &encrypted_use)
}

fn check_incomplete_basis(sources: &SourceUses, encrypted_use: &SourceUse) -> TestResult {
    let incomplete = sources.shared_origin(&[], false);
    let joined = sources.value_with_shared_origin(
        SourceValue::Present(()),
        &incomplete,
        std::slice::from_ref(encrypted_use),
        true,
    )?;
    assert_eq!(
        joined.origin().contains_encrypted_source(),
        Some(true),
        "a known encrypted use survives incomplete shared coverage"
    );
    assert!(
        !joined.origin().source_set_complete(),
        "additions cannot make incomplete shared coverage complete"
    );
    Ok(())
}

#[test]
fn shared_origin_refuses_another_inventory_even_at_an_equal_coordinate() -> TestResult {
    let local = SourceUses::new();
    let foreign = SourceUses::new();
    let left_scope = local.observe(SourceInput::Unknown, InputScope::Payload)?;
    let right_scope = foreign.observe(SourceInput::Unknown, InputScope::Payload)?;
    let left = evidence(&left_scope, "same", SourceUseRole::Selector)?;
    let right = evidence(&right_scope, "same", SourceUseRole::Selector)?;
    assert_eq!(
        left.reference(),
        right.reference(),
        "wire identifiers match across independent inventories"
    );
    let wrong = foreign.shared_origin(std::slice::from_ref(&right), true);
    assert!(
        matches!(
            local.value_with_shared_origin(
                SourceValue::Present(()),
                &wrong,
                std::slice::from_ref(&left),
                true
            ),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "an equal wire coordinate cannot admit a foreign shared origin"
    );
    let shared = local.shared_origin(std::slice::from_ref(&left), true);
    let joined =
        local.value_with_shared_origin(SourceValue::Present(()), &shared, &[right], true)?;
    assert_eq!(
        joined.origin().source_uses_refs(),
        std::slice::from_ref(left.reference()),
        "a foreign addition is excluded"
    );
    assert!(
        !joined.origin().source_set_complete(),
        "a foreign contributor leaves the complete set unproved"
    );
    Ok(())
}

fn evidence(scope: &SourceScope, field: &str, role: SourceUseRole) -> TestResult<SourceUse> {
    Ok(scope.at(
        SourceLocation::DataForge {
            record_index: 0,
            field: field.to_owned(),
        },
        role,
    )?)
}

fn archive_scope(
    directory: &Path,
    sources: &SourceUses,
    encrypted: bool,
) -> TestResult<SourceScope> {
    let path = directory.join(if encrypted {
        "encrypted.p4k"
    } else {
        "clear.p4k"
    });
    let entry = TestEntry::stored("Data\\Game2.dcb", "original evidence");
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = archive.entries().first().ok_or("actual original Entry")?;
    let origin = archive.entry_origin(entry).ok_or("actual Entry origin")?;
    Ok(sources.observe(SourceInput::Archive(&origin), InputScope::Payload)?)
}
