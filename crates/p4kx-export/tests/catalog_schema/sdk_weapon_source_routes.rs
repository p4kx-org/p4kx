// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{
    WeaponSourceSnapshot, with_weapon_recoil_definitions, with_weapon_sources_from,
};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

use super::super::{
    AMMO_BINDINGS, AMMO_CONTAINERS, Route, TestResult, WEAPON_ACTIONS, WEAPON_RECOIL,
};

use super::projectile_ammo_fixture as fixture;

#[path = "sdk_ammo_binding_witness.rs"]
mod ammo_binding_witness;

pub(super) fn read_recoil(archive: &Archive) -> TestResult<Route> {
    with_weapon_recoil_definitions(archive, CatalogRecordSelection::All, None, |snapshot| {
        Route::queried_rows(
            WEAPON_RECOIL,
            snapshot.rows(),
            "physical definition selection available; installed and complete producer parity open",
        )
    })?
}

pub(super) fn read() -> TestResult<[Route; 3]> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("sdk_weapon_sources.p4k");
    let database = fixture::build(fixture::Case::Unique);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut owners = forge.records().filter(|record| {
        record
            .type_name()
            .is_ok_and(|source| source == "EntityClassDefinition")
    });
    let (Some(owner), None) = (owners.next(), owners.next()) else {
        return Err("the fixture requires one physical weapon owner".into());
    };
    let catalog = ammo_binding_witness::normal(&archive, &entry)?;
    with_weapon_sources_from(&archive, &entry, owner.index(), None, |snapshot| {
        ammo_binding_witness::verify(&archive, &entry, &snapshot, &catalog)?;
        verify(&snapshot, owner.index())
    })?
}

fn verify(snapshot: &WeaponSourceSnapshot<'_>, record_index: u32) -> TestResult<[Route; 3]> {
    const OWNERS_OPEN: &str = "physical Item query and synthetic Catalog parity available; owner enumeration and installed parity open";
    const DEPENDENCIES_OPEN: &str = "physical Item query and synthetic Catalog parity available; linked dependency rows, owner enumeration and installed parity open";

    let [owner] = snapshot.rows() else {
        return Err("the scoped query must return one physical Item".into());
    };
    let [weapon] = owner.weapon_actions.as_slice() else {
        return Err("the physical Item has one original weapon component".into());
    };
    let [container] = owner.ammo_containers.as_slice() else {
        return Err("the physical Item has one original ammo container".into());
    };
    if weapon.owner().record_index().value() != Some(&record_index)
        || container.owner().record_index().value() != Some(&record_index)
        || weapon.owner().component_ordinal().value() != Some(&1)
        || container.owner().component_ordinal().value() != Some(&2)
        || owner.ammo_bindings.len() != 2
        || owner
            .ammo_bindings
            .iter()
            .any(|binding| binding.owner().record_index().value() != Some(&record_index))
    {
        return Err("the original weapon, container and binding occurrences must agree".into());
    }
    let capacity = container.max_ammo_count();
    let origin = capacity.origin();
    if capacity.value() != Some(&3)
        || origin.contains_encrypted_source() != Some(true)
        || !origin.source_set_complete()
        || !snapshot.source_inventory().iter_uses().any(|used| {
            used.scope() == InputScope::Payload && origin.source_uses_refs().contains(used.id())
        })
    {
        return Err("the exact container capacity needs its encrypted source evidence".into());
    }

    Ok([
        Route::queried_rows(WEAPON_ACTIONS, &owner.weapon_actions, OWNERS_OPEN)?,
        Route::queried_rows(AMMO_CONTAINERS, &owner.ammo_containers, DEPENDENCIES_OPEN)?,
        Route::queried_rows(
            AMMO_BINDINGS,
            &owner.ammo_bindings,
            "typed owner and physical Ammo target joins match the synthetic producer; owner enumeration, installed parity and standalone memory open",
        )?,
    ])
}
