// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::{
    Archive, InputScope, SourceCoordinateOmission, SourceEvidenceError, SourceInput,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

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
