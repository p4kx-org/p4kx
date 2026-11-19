// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AmmoSelection, ProjectileAmmoChoice, ProjectileAmmoSnapshot, with_projectile_ammo,
};
use p4kx_extraction::{Archive, DataForge, Filter};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{AMMUNITION, PROJECTILE_AMMUNITION, Route, TestResult};

#[expect(
    dead_code,
    reason = "The schema witness reuses only the successful linked weapon fixture."
)]
#[path = "../it/catalog_weapon_linked_ammo_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 2]> {
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
    with_projectile_ammo(&archive, record_index, None, |snapshot| {
        reached(&snapshot, record_index)
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
