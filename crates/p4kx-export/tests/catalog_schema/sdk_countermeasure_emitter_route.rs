// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::{
    ProjectileAmmoOwner, ProjectileAmmoRole, with_projectile_ammo_from,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry, Filter};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::Value;

use super::super::{COUNTERMEASURE_EMITTERS, Route, TestResult};

use super::projectile_ammo_fixture as fixture;

const WEAPON_PATH: &str = "*/fixture_projectile_own_route";

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = open(fixture::Case::Countermeasure)?;
    let filter = Filter::parse(WEAPON_PATH)?;
    let catalog = build_catalog(
        &archive,
        &CatalogOptions {
            filter: Some(&filter),
            ..CatalogOptions::new(UNIX_EPOCH)
        },
    )?;
    let document: Value = serde_json::from_slice(catalog.json())?;
    let table = document
        .pointer(COUNTERMEASURE_EMITTERS)
        .ok_or("the canonical emitter table")?;
    let route = with_weapon(&archive, &filter, |index, owner| {
        verify_roles(index, owner)?;
        assert_eq!(
            table.get("rows"),
            Some(&serde_json::to_value(&owner.countermeasure_emitters)?),
            "the filtered Catalog preserves the same rows and value origins"
        );
        Route::queried_rows(
            COUNTERMEASURE_EMITTERS,
            &owner.countermeasure_emitters,
            "physical Item query and encrypted synthetic producer parity witnessed; owner enumeration and installed parity open",
        )
    })?;

    let (_directory, archive) = open(fixture::Case::Unique)?;
    with_weapon(&archive, &filter, |_, owner| {
        assert_eq!(
            owner.ammo_definitions.len(),
            2,
            "the control weapon still reaches ordinary projectile definitions"
        );
        let [] = owner.countermeasure_emitters.as_slice() else {
            return Err("ordinary projectiles must not become countermeasure emitters".into());
        };
        Ok(())
    })?;
    Ok(route)
}

fn verify_roles(index: u32, owner: &ProjectileAmmoOwner<'_>) -> TestResult<()> {
    let [primary, secondary] = owner.countermeasure_emitters.as_slice() else {
        return Err("each distinct projectile role supplies exactly one emitter".into());
    };
    for (emitter, role) in [
        (primary, ProjectileAmmoRole::Primary),
        (secondary, ProjectileAmmoRole::Secondary),
    ] {
        assert_eq!(
            emitter.owner().record_index().value(),
            Some(&index),
            "the emitter names the queried physical Item"
        );
        let cell = emitter.route();
        assert_eq!(
            cell.origin().contains_encrypted_source(),
            Some(true),
            "the route marks its encrypted original Archive Entry"
        );
        assert!(
            cell.origin().source_set_complete(),
            "the route retains all original contributors"
        );
        let route = cell.value().ok_or("the proved original emitter route")?;
        assert_eq!(route.ammo_role().value(), Some(&role), "the authored role");
        assert_eq!(
            route.container_record_index().value(),
            Some(&index),
            "the container belongs to the same physical owner"
        );
        assert_eq!(
            route.container_component_ordinal().value(),
            Some(&2),
            "the original container component slot is retained"
        );
        assert!(
            owner.ammo_definitions.iter().any(|ammo| {
                route.ammo_record_index().value() == ammo.source().record_index().value()
            }),
            "the linked physical ammunition definition is in the same query"
        );
    }
    Ok(())
}

fn open(case: fixture::Case) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("countermeasure_emitters.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::build(case).build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    Ok((directory, archive))
}

fn with_weapon<T>(
    archive: &Archive,
    filter: &Filter,
    inspect: impl FnOnce(u32, &ProjectileAmmoOwner<'_>) -> TestResult<T>,
) -> TestResult<T> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let mut selected = forge
        .records()
        .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)));
    let (Some(weapon), None) = (selected.next(), selected.next()) else {
        return Err("exactly one physical fixture Item".into());
    };
    let index = weapon.index();
    with_projectile_ammo_from(archive, &entry, index, None, |snapshot| {
        let [owner] = snapshot.rows() else {
            return Err("the selected Item's original ammunition routes".into());
        };
        assert_eq!(
            snapshot.coverage().unreadable_bodies,
            [] as [u32; 0],
            "the selected original Item is readable"
        );
        assert_eq!(
            owner.dependency_coverage.unreadable_bodies,
            [] as [u32; 0],
            "all admitted original dependencies are readable"
        );
        inspect(index, owner)
    })?
}
