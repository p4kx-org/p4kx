// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, FieldPath, SourceUses};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::origin_addresses::OriginAddresses;
use crate::origin_contributions::OriginContributions;
use crate::origin_state::OriginState;

#[test]
fn exact_material_control_overrides_its_source_family_without_losing_siblings()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = SourceUses::new();
    let mut addresses = OriginAddresses::default();
    addresses.insert_subtree(
        &FieldPath::parse("/value")?,
        OriginContributions::unknown(&owner),
    )?;
    addresses.insert_subtree(
        &FieldPath::parse("/value/literal")?,
        OriginContributions::independent(&owner),
    )?;
    assert!(
        !addresses
            .visit("/value/source/raw_bits")
            .ok_or("source family")?
            .complete
    );
    assert!(
        addresses
            .visit("/value/literal")
            .ok_or("actual literal")?
            .complete
    );
    assert!(
        addresses
            .visit("/value/literal/value")
            .ok_or("literal family")?
            .complete
    );
    addresses.require_consumed()?;
    Ok(())
}

#[test]
fn visiting_a_material_family_cannot_consume_a_nonexistent_prepared_child()
-> Result<(), Box<dyn std::error::Error>> {
    let owner = SourceUses::new();
    let mut addresses = OriginAddresses::default();
    addresses.insert_subtree(
        &FieldPath::parse("/value")?,
        OriginContributions::unknown(&owner),
    )?;
    addresses.insert_subtree(
        &FieldPath::parse("/value/missing")?,
        OriginContributions::independent(&owner),
    )?;
    assert!(addresses.visit("/value").is_some());
    assert!(addresses.visit("/value/present").is_some());
    assert!(
        addresses.require_consumed().is_err(),
        "family inheritance does not grant nonexistent payload addresses"
    );
    Ok(())
}

#[test]
fn detached_material_lookup_observer_keeps_original_archive_ownership()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let first = folder.path().join("first.p4k");
    let second = folder.path().join("second.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("same.ext", b"Version(1)"))
        .write_to(&first)?;
    std::fs::copy(&first, &second)?;
    let first = Archive::open(first)?;
    let second = Archive::open(second)?;
    let mut state = OriginState::new(u64::MAX);
    state.observe_root(&first, std::fs::metadata(first.path())?.len())?;
    let observer = state.observer()?;
    let entry = first.entries().first().ok_or("original entry")?;
    let owned = first
        .entry_origin(entry)
        .ok_or("original owned observation")?;
    let input = observer.original(&owned)?;
    assert!(input.owner.is_same_inventory(&state.owner));
    assert!(
        input
            .scope
            .matches_entry(&first.observed_entry(entry).ok_or("original view")?)?
    );
    let foreign = second.entries().first().ok_or("equal foreign entry")?;
    assert!(observer.entry(&second, foreign).is_err());
    assert!(
        observer
            .original(
                &second
                    .entry_origin(foreign)
                    .ok_or("foreign owned observation")?
            )
            .is_err(),
        "equal path, metadata and bytes cannot restore the original root binding"
    );
    Ok(())
}
