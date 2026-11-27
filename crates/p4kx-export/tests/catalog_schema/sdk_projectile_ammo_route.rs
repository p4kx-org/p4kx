// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AmmoSelection, DpsUnit, ProjectileAmmoChoice, ProjectileAmmoSnapshot, WeaponMagazineOutcome,
    with_projectile_ammo, with_weapon_combat_bases, with_weapon_magazine_resolutions,
};
use p4kx_extraction::{Archive, DataForge, Filter};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{
    AMMUNITION, MAGAZINE_RESOLUTION, PROJECTILE_AMMUNITION, Route, TestResult, WEAPON_COMBAT,
};

#[expect(
    dead_code,
    reason = "The schema witness reuses only the successful linked weapon fixture."
)]
#[path = "../it/catalog_weapon_linked_ammo_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 4]> {
    let bytes =
        fixture::with_ammo_names(fixture::Case::Unique, "debug_primary", "debug_secondary").build();
    let record_index = {
        let forge = DataForge::open(&bytes)?;
        let filter = Filter::parse(fixture::FILTER)?;
        let mut candidates = forge
            .records()
            .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)));
        let (Some(weapon), None) = (candidates.next(), candidates.next()) else {
            return Err("the fixture requires one physical weapon owner".into());
        };
        weapon.index()
    };
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("sdk_projectile_ammo.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let [projectile, ammo] = with_projectile_ammo(&archive, record_index, None, |snapshot| {
        reached(&snapshot, record_index)
    })??;
    Ok([
        projectile,
        ammo,
        combat(&archive, record_index)?,
        magazine(&archive, record_index)?,
    ])
}

fn magazine(archive: &Archive, weapon: u32) -> TestResult<Route> {
    with_weapon_magazine_resolutions(archive, weapon, None, |snapshot| {
        let [row] = snapshot.rows() else {
            return Err("one observed legacy route for the physical weapon".into());
        };
        if row.owner().record_index().value() != Some(&weapon)
            || !matches!(
                row.outcome().value(),
                Some(WeaponMagazineOutcome::LinkedContainer { .. })
            )
        {
            return Err("one original linked primary magazine route".into());
        }
        Route::queried_rows(
            MAGAZINE_RESOLUTION,
            snapshot.rows(),
            "physical Item query available; owner enumeration and installed parity open",
        )
    })?
}

fn combat(archive: &Archive, weapon: u32) -> TestResult<Route> {
    with_weapon_combat_bases(archive, weapon, None, |snapshot| {
        let [owner] = snapshot.rows() else {
            return Err("the public combat query must read one physical weapon".into());
        };
        if owner.combat_bases.len() != 2
            || owner.combat_bases.iter().enumerate().any(|(index, row)| {
                row.owner().record_index().value() != Some(&weapon)
                    || row.mode_index().value().copied() != u32::try_from(index).ok()
                    || row.formula_version() != 1
                    || !matches!(row.units().nominal_dps(), DpsUnit::DamagePointsPerSecond)
            })
        {
            return Err(
                "the SDK combat rows must retain both original slots and their contract".into(),
            );
        }
        Route::queried_rows(
            WEAPON_COMBAT,
            &owner.combat_bases,
            "physical Item combat query available; owner enumeration and installed parity open",
        )
    })?
}

fn reached(snapshot: &ProjectileAmmoSnapshot<'_>, weapon: u32) -> TestResult<[Route; 2]> {
    let [owner] = snapshot.rows() else {
        return Err("the public query must read one physical weapon".into());
    };
    let [primary, secondary] = owner.selections.as_slice() else {
        return Err("the public query must retain both physical launcher occurrences".into());
    };
    let [first, second] = owner.ammo_definitions.as_slice() else {
        return Err("the public query must retain both reached Ammo definitions".into());
    };
    for (selection, definition) in [(primary, first), (secondary, second)] {
        let route = selection.selection();
        let Some(ProjectileAmmoChoice::SelectedLinkedMagazine {
            ammo_record_index, ..
        }) = route.value()
        else {
            return Err("the source must prove a linked magazine route".into());
        };
        if !route.origin().source_set_complete()
            || selection.owner().record_index().value() != Some(&weapon)
            || ammo_record_index.value() != definition.source().record_index().value()
            || definition.selection() != AmmoSelection::Dependency
        {
            return Err(
                "the SDK route, physical owner and reached Ammo evidence must agree".into(),
            );
        }
    }
    if !snapshot.coverage().unreadable_bodies.is_empty()
        || !owner.dependency_coverage.unreadable_bodies.is_empty()
    {
        return Err("the known readable fixture must cover all admitted bodies".into());
    }
    Ok([
        Route::queried_rows(
            PROJECTILE_AMMUNITION,
            &owner.selections,
            "physical Item query available; owner enumeration and installed parity open",
        )?,
        Route::queried_rows(
            AMMUNITION,
            &owner.ammo_definitions,
            "independent and projectile-reached queries available; complete dependency enumeration and installed parity open",
        )?,
    ])
}
