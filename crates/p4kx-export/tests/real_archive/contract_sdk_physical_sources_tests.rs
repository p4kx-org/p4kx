// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{
    Archive, InputScope, PhysicalSourceKind, PhysicalSourceRef, SourceInput, SourceLocation,
    SourceScope, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Comparison, TestResult, original_source};

fn archive(folder: &Path, name: &str, entry_path: &str) -> TestResult<Archive> {
    let path = folder.join(name);
    ArchiveBuilder::new()
        .entry(TestEntry::stored(entry_path, "same bytes"))
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

fn observe(archive: &Archive, uses: &SourceUses) -> TestResult<PhysicalSourceRef> {
    let entry = archive.entries().first().ok_or("one original Entry")?;
    let observed = archive
        .observed_entry(entry)
        .ok_or("actual observed Entry")?;
    Ok(uses
        .observe(SourceInput::ArchiveView(observed), InputScope::Payload)?
        .source_ref()
        .clone())
}

#[test]
fn physical_source_witness_resolves_full_and_compact_aliases_instead_of_equating_wire_refs()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let source = archive(folder.path(), "source.p4k", "Data/Game2.dcb")?;
    let full = SourceUses::new();
    let expected_ref = observe(&source, &full)?;
    let compact = SourceUses::with_compact_coordinates();
    let actual_ref = observe(&source, &compact)?;
    assert_eq!(
        expected_ref, actual_ref,
        "the controlled counterexample changes only the use-reference encoding"
    );
    let expected = full.freeze()?;
    let actual = compact.freeze()?;
    let left = expected
        .iter_sources()
        .find(|source| source.id() == &expected_ref)
        .ok_or("expected source")?;
    let right = actual
        .iter_sources()
        .find(|source| source.id() == &actual_ref)
        .ok_or("actual source")?;
    assert_ne!(
        left, right,
        "whole-row Eq includes document-local references and cannot prove a semantic source mismatch"
    );
    let (PhysicalSourceKind::ArchiveEntry(left), PhysicalSourceKind::ArchiveEntry(right)) =
        (left.kind(), right.kind())
    else {
        return Err("two original physical Entries".into());
    };
    assert_eq!(
        left.path().value(),
        right.path().value(),
        "the original source path facts are identical"
    );
    assert_ne!(
        left.path().origin().source_uses_refs(),
        right.path().origin().source_uses_refs(),
        "the same physical metadata has different Full and Compact use names"
    );
    assert_eq!(
        original_source(&expected, &actual, &expected_ref)?,
        actual_ref,
        "semantic metadata/ancestor checks map the actual source without changing any evidence"
    );
    Ok(())
}

#[test]
fn physical_source_witness_maps_local_source_names_after_unrelated_input_registration() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let source = archive(folder.path(), "source.p4k", "Data/Game2.dcb")?;
    let other = archive(folder.path(), "other.p4k", "Data/other.bin")?;
    let full = SourceUses::new();
    let expected_ref = observe(&source, &full)?;
    let compact = SourceUses::with_compact_coordinates();
    observe(&other, &compact)?;
    let actual_ref = observe(&source, &compact)?;
    assert_ne!(
        expected_ref, actual_ref,
        "source names belong to their own input registration order"
    );
    assert_eq!(
        original_source(&full.freeze()?, &compact.freeze()?, &expected_ref)?,
        actual_ref,
        "source and use aliases are mapped only after all original typed facts and provenance match"
    );
    Ok(())
}

fn nested(folder: &Path, name: &str, encrypted: bool) -> TestResult<Archive> {
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", "same bytes"))
        .build();
    let entry = TestEntry::stored("Data/outer.socpak", bytes);
    let entry = if encrypted { entry.encrypted() } else { entry };
    let path = folder.join(name);
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let root = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = root
        .entries()
        .first()
        .ok_or("one original containing Entry")?;
    Ok(root.open_container(entry)?)
}

#[test]
fn physical_source_witness_keeps_encrypted_ancestor_identity_and_rejects_a_plain_counterpart()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let encrypted = nested(folder.path(), "encrypted.p4k", true)?;
    let plain = nested(folder.path(), "plain.p4k", false)?;
    let full = SourceUses::new();
    let expected_ref = observe(&encrypted, &full)?;
    let same = SourceUses::with_compact_coordinates();
    let actual_ref = observe(&encrypted, &same)?;
    let wrong = SourceUses::with_compact_coordinates();
    observe(&plain, &wrong)?;
    let expected = full.freeze()?;
    assert_eq!(
        original_source(&expected, &same.freeze()?, &expected_ref)?,
        actual_ref,
        "all physical ancestors and inherited metadata encryption survive reference encoding changes"
    );
    let source = expected
        .iter_sources()
        .find(|source| source.id() == &expected_ref)
        .ok_or("original nested source")?;
    assert_eq!(
        source.ancestors().len(),
        1,
        "this original leaf has an actual containing Entry"
    );
    let PhysicalSourceKind::ArchiveEntry(entry) = source.kind() else {
        return Err("actual leaf Entry facts".into());
    };
    assert_eq!(
        entry.path().origin().contains_encrypted_source(),
        Some(true),
        "metadata genuinely depends on its encrypted containing Entry"
    );
    assert!(
        original_source(&expected, &wrong.freeze()?, &expected_ref).is_err(),
        "a different containing-source encryption declaration cannot be normalized away"
    );
    let standalone = archive(folder.path(), "standalone.p4k", "Data/Game2.dcb")?;
    let missing_ancestor = SourceUses::with_compact_coordinates();
    observe(&standalone, &missing_ancestor)?;
    assert!(
        original_source(&expected, &missing_ancestor.freeze()?, &expected_ref).is_err(),
        "a matching leaf path cannot substitute for its real missing ancestor"
    );
    Ok(())
}

#[test]
fn physical_source_witness_rejects_ambiguous_equal_metadata_from_distinct_inputs() -> TestResult {
    let folder = tempfile::tempdir()?;
    let first = archive(folder.path(), "first.p4k", "Data/Game2.dcb")?;
    let second = archive(folder.path(), "second.p4k", "Data/Game2.dcb")?;
    let full = SourceUses::new();
    let expected_ref = observe(&first, &full)?;
    let ambiguous = SourceUses::with_compact_coordinates();
    observe(&first, &ambiguous)?;
    observe(&second, &ambiguous)?;
    assert!(
        original_source(&full.freeze()?, &ambiguous.freeze()?, &expected_ref).is_err(),
        "identical descriptive metadata does not authorize choosing between distinct physical inputs"
    );
    Ok(())
}

#[test]
fn physical_source_witness_does_not_discard_metadata_roles_coordinates_or_completeness()
-> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = archive(folder.path(), "source.p4k", "Data/Game2.dcb")?;
    let full = SourceUses::new();
    let compact = SourceUses::with_compact_coordinates();
    let entry = archive.entries().first().ok_or("actual Entry")?;
    let view = archive
        .observed_entry(entry)
        .ok_or("original observed Entry")?;
    let expected_scope = full.observe(
        SourceInput::ArchiveView(view),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let actual_scope = compact.observe(
        SourceInput::ArchiveView(view),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let original = expected_scope.at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let expected = full.value(
        SourceValue::Present(entry.path().to_owned()),
        &[original],
        true,
    );
    let mismatches = wrong_origins(&compact, &actual_scope, entry.path())?;
    let expected_inventory = full.freeze()?;
    let actual_inventory = compact.freeze()?;
    let comparison = Comparison::read(&expected_inventory, &actual_inventory)?;
    comparison.verify(&expected_inventory, &actual_inventory)?;
    for mismatch in mismatches {
        assert!(
            comparison
                .origin(expected.origin(), mismatch.origin())
                .is_err(),
            "matching field values cannot hide a changed original coordinate, contributor role or incomplete marker"
        );
    }
    Ok(())
}

fn wrong_origins(
    uses: &SourceUses,
    scope: &SourceScope,
    path: &str,
) -> TestResult<Vec<SourcedValue<String>>> {
    let mut mismatches = Vec::new();
    for (field, role) in [
        ("generalPurposeFlags", SourceUseRole::Payload),
        ("path", SourceUseRole::Selector),
    ] {
        let altered = scope.at(
            SourceLocation::Metadata {
                field: field.to_owned(),
            },
            role,
        )?;
        mismatches.push(uses.value(SourceValue::Present(path.to_owned()), &[altered], true));
    }
    let same = scope.at(
        SourceLocation::Metadata {
            field: "path".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    mismatches.push(uses.value(SourceValue::Present(path.to_owned()), &[same], false));
    Ok(mismatches)
}
