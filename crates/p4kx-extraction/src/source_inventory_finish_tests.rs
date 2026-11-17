// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::{
    Archive, FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
};

use super::SourceInventoryFreezeError;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn use_at(sources: &SourceUses, record_index: u32) -> Result<SourceUse, SourceEvidenceError> {
    observe_in(
        &sources.observe(SourceInput::Unknown, InputScope::Payload)?,
        record_index,
    )
}

fn observe_in(scope: &SourceScope, record_index: u32) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::DataForge {
            record_index,
            field: "source".into(),
        },
        SourceUseRole::Payload,
    )
}

#[test]
fn finishing_moves_sorted_rows_and_preserves_captured_value_binding() -> TestResult {
    let sources = SourceUses::with_compact_coordinates();
    let same = sources.clone();
    assert_eq!(sources.registered_use_count()?, 0, "empty capture");
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let first = observe_in(&scope, 9)?;
    let second = use_at(&sources, 1)?;
    observe_in(&scope, 9)?;
    assert_eq!(
        sources.registered_use_count()?,
        2,
        "repeat capture does not add a registered use",
    );
    let foreign = use_at(&SourceUses::with_compact_coordinates(), 9)?;
    let own = [second.clone(), first];
    let mixed = [second.clone(), foreign];
    let previous = sources.freeze()?;
    assert_eq!(
        same.registered_use_count()?,
        previous.use_count(),
        "the in-flight count equals the frozen row count",
    );
    let expected = sources.value(SourceValue::Present(42), &own, true);
    let expected_mixed = sources.value(SourceValue::Present(42), &mixed, true);

    let inventory = sources.finish_with(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(
        inventory, previous,
        "same inventory as the repeatable snapshot"
    );
    assert!(
        inventory
            .iter_uses()
            .any(|row| std::ptr::eq(row, second.description())),
        "capture keeps the original registered Arc row"
    );
    assert_eq!(same.value(SourceValue::Present(42), &own, true), expected);
    assert_eq!(
        same.value(SourceValue::Present(42), &mixed, true),
        expected_mixed,
        "foreign evidence is still rejected during deferred value binding"
    );
    assert!(
        !expected_mixed.origin().source_set_complete(),
        "a foreign contributor does not become complete evidence"
    );
    assert!(
        matches!(same.freeze(), Err(SourceEvidenceError::SealedInventory)),
        "the terminal finish seals every cloned registry handle"
    );
    Ok(())
}

#[test]
fn compacted_and_held_uses_keep_wire_order_identity_and_origin_facts() -> TestResult {
    let sources = SourceUses::with_compact_coordinates();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let held = observe_in(&scope, 100)?;
    let cold = observe_in(&scope, 1)?;
    let cold_description = Arc::downgrade(&cold.description);
    let held_origin = sources.origin(std::slice::from_ref(&held), true);
    let cold_origin = sources.origin(std::slice::from_ref(&cold), false);
    let expected_origin = sources.origin(&[held.clone(), cold.clone()], false);
    let expected_wire = serde_json::to_vec(&sources.freeze()?)?;
    drop(cold);

    assert_interruption_keeps_pending_uses(&sources)?;
    sources.compact_record_uses(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(
        sources.packed_use_count()?,
        1,
        "only the unheld row is packed"
    );
    assert_eq!(sources.registered_use_count()?, 2);
    assert_eq!(
        cold_description.strong_count(),
        0,
        "the compacted row releases its former description"
    );
    assert_eq!(
        Arc::strong_count(&held.description),
        2,
        "the registry and the held observation share one description"
    );
    let aggregate = sources.origin_with_origins([&held_origin, &cold_origin], true)?;
    assert_eq!(
        aggregate, expected_origin,
        "packed uses apply the same origin facts"
    );

    let repeated = observe_in(&scope, 1)?;
    let cold_ref = cold_origin
        .source_uses_refs()
        .first()
        .ok_or("packed reference")?;
    assert!(
        std::ptr::eq(repeated.reference().as_str(), cold_ref.as_str()),
        "re-registration reuses the canonical reference"
    );
    assert!(
        std::ptr::eq(
            repeated.description().source().as_str(),
            held.description().source().as_str(),
        ),
        "re-registration reuses the original physical source reference"
    );
    let same_held = observe_in(&scope, 100)?;
    assert!(std::ptr::eq(same_held.description(), held.description()));
    drop(repeated);
    sources.compact_record_uses(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(sources.packed_use_count()?, 1);

    assert_registry_rejects_foreign_origin(&sources, &cold_origin)?;
    assert_original_inventory(&sources, &held, &expected_wire)
}

fn assert_registry_rejects_foreign_origin(
    sources: &SourceUses,
    origin: &FieldOrigin,
) -> TestResult {
    let other = SourceUses::with_compact_coordinates();
    let foreign = use_at(&other, 1)?;
    let foreign_origin = other.origin(&[foreign], true);
    assert_eq!(
        foreign_origin.source_uses_refs(),
        origin.source_uses_refs(),
        "the two authorities use the same document-local ID"
    );
    assert!(
        matches!(
            sources.origin_with_origins([&foreign_origin], true),
            Err(SourceEvidenceError::ForeignEvidence)
        ),
        "a matching ID under a foreign authority is rejected"
    );
    Ok(())
}

fn assert_original_inventory(
    sources: &SourceUses,
    held: &SourceUse,
    expected_wire: &[u8],
) -> TestResult {
    let snapshot = sources.freeze()?;
    let finished = sources.finish_with(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(serde_json::to_vec(&snapshot)?, expected_wire);
    assert_eq!(serde_json::to_vec(&finished)?, expected_wire);
    assert_eq!(snapshot, finished);
    assert!(
        finished
            .iter_uses()
            .any(|row| std::ptr::eq(row, held.description()))
    );
    assert!(
        finished.iter_uses().eq(finished.uses()),
        "both compatibility views expose the same complete sorted rows"
    );
    Ok(())
}

fn assert_interruption_keeps_pending_uses(sources: &SourceUses) -> TestResult {
    let interrupted =
        sources.compact_record_uses(|| Err(SourceInventoryFreezeError::Interrupted(())));
    assert!(matches!(
        interrupted,
        Err(SourceInventoryFreezeError::Interrupted(()))
    ));
    assert_eq!(sources.packed_use_count()?, 0);
    Ok(())
}

#[test]
fn packed_origins_preserve_encrypted_clear_and_unknown_source_evidence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = encryption_fixture(directory.path())?;
    let sources = SourceUses::with_compact_coordinates();
    let observe = |input, role| -> Result<_, SourceEvidenceError> {
        let used = sources.observe(input, InputScope::Payload)?.at(
            SourceLocation::DataForge {
                record_index: 7,
                field: "source".into(),
            },
            role,
        )?;
        Ok(sources.origin(&[used], true))
    };
    let clear = observe(
        archive_input(&archive, "clear.dcb")?,
        SourceUseRole::Payload,
    )?;
    let encrypted = observe(
        archive_input(&archive, "encrypted.dcb")?,
        SourceUseRole::Selector,
    )?;
    let unknown = observe(SourceInput::Unknown, SourceUseRole::Payload)?;
    let cases: [(&[_], bool, Option<bool>); 5] = [
        (&[&clear], true, Some(false)),
        (&[&unknown], true, None),
        (&[&clear, &unknown], true, None),
        (&[&clear, &encrypted, &unknown], true, Some(true)),
        (&[&encrypted, &unknown], false, Some(true)),
    ];
    let previous = cases
        .iter()
        .map(|(parts, complete, _)| sources.origin_with_origins(parts.iter().copied(), *complete))
        .collect::<Result<Vec<_>, _>>()?;
    let previous_wire = serde_json::to_vec(&sources.freeze()?)?;

    sources.compact_record_uses(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(sources.packed_use_count()?, 3, "all three uses are packed");
    for ((parts, complete, marker), original) in cases.into_iter().zip(previous) {
        let origin = sources.origin_with_origins(parts.iter().copied(), complete)?;
        assert_eq!(origin.contains_encrypted_source(), marker, "source marker");
        assert_eq!(
            origin.source_set_complete(),
            complete,
            "source completeness"
        );
        assert_eq!(origin, original, "exact contributing source references");
    }
    let finished = sources.finish_with(|| Ok::<(), SourceEvidenceError>(()))?;
    assert_eq!(
        serde_json::to_vec(&finished)?,
        previous_wire,
        "the packed inventory retains each original role and encryption state"
    );
    Ok(())
}

fn archive_input<'a>(archive: &'a Archive, path: &str) -> Result<SourceInput<'a>, &'static str> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .and_then(|entry| archive.observed_entry(entry))
        .map(SourceInput::ArchiveView)
        .ok_or("owned Archive Entry")
}

fn encryption_fixture(directory: &std::path::Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = directory.join("encryption.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("clear.dcb", b"clear fixture"))
        .entry(TestEntry::deflated("encrypted.dcb", b"encrypted fixture").encrypted())
        .write_to(&path)?;
    Ok(Archive::open(&path)?)
}

#[test]
fn interruption_before_seal_leaves_registration_available() -> TestResult {
    let sources = SourceUses::new();
    use_at(&sources, 1)?;
    let cancelled = sources.finish_with(|| Err(SourceInventoryFreezeError::Interrupted(())));
    assert!(
        matches!(cancelled, Err(SourceInventoryFreezeError::Interrupted(()))),
        "the first checkpoint prevents the terminal operation"
    );
    use_at(&sources, 2)?;
    assert_eq!(
        sources.freeze()?.use_count(),
        2,
        "registration remains open"
    );
    Ok(())
}

#[test]
fn interruption_while_packing_leaves_the_registry_sealed() -> TestResult {
    let sources = SourceUses::with_compact_use_refs();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    use_at(&sources, 1)?;
    let mut check = 0;
    let cancelled = sources.finish_with(|| {
        check += 1;
        if check == 1 {
            Ok(())
        } else {
            Err(SourceInventoryFreezeError::Interrupted(()))
        }
    });
    assert!(
        matches!(cancelled, Err(SourceInventoryFreezeError::Interrupted(()))),
        "interruption cannot return a partially packed inventory"
    );
    assert_eq!(check, 2, "the operation entered terminal row packing");
    assert!(
        matches!(
            sources.observe(SourceInput::Unknown, InputScope::Payload),
            Err(SourceEvidenceError::SealedInventory)
        ),
        "a new source cannot enter the sealed inventory"
    );
    assert!(
        matches!(
            scope.at(
                SourceLocation::Metadata {
                    field: "path".into()
                },
                SourceUseRole::Payload,
            ),
            Err(SourceEvidenceError::SealedInventory)
        ),
        "an existing scope cannot register a new use"
    );
    assert!(
        matches!(scope.ancestor(0), Err(SourceEvidenceError::SealedInventory)),
        "inspection cannot read an incomplete or emptied index"
    );
    Ok(())
}
