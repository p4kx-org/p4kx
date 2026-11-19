// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AmmoSelection, ProjectileAmmoChoice, ProjectileAmmoOwner, ProjectileAmmoRole,
    ProjectileAmmoSnapshot, with_projectile_ammo, with_projectile_ammo_from,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, DataForgeEntry};
use p4kx_testkit::TestRecord;
use serde_json::{Value, json};

use super::{ROWS, fixture};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const AMMO: &str = "/definitions/ammoDefinitions/rows";

#[test]
fn queried_external_magazine_routes_and_reached_ammo_match_the_catalog() -> TestResult {
    let unused = 0x819;
    let database =
        fixture::with_ammo_names(fixture::Case::Unique, "debug_primary", "debug_secondary").record(
            TestRecord::new(
                unused,
                "AmmoParams",
                "AmmoParams.Unused",
                "libs/foundry/records/ammoparams/fixture_unused.xml",
            )
            .with_instance_index(u16::MAX),
        );
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let weapon = physical(&forge, fixture::WEAPON)?;
    let magazine = physical(&forge, fixture::MAGAZINE)?;
    let ammo = [
        physical(&forge, fixture::PRIMARY)?,
        physical(&forge, fixture::SECONDARY)?,
    ];
    let unused = physical(&forge, unused)?;
    let source = forge.record(unused).ok_or("unused physical Ammo Record")?;
    assert!(
        source.tree().is_err(),
        "the unrelated Ammo body really fails"
    );
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let mut options = common::options();
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let catalog = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
        with_projectile_ammo(&archive, weapon, None, |snapshot| {
            assert_projection(&snapshot, &catalog, magazine, ammo, encrypted)
        })??;
    }
    Ok(())
}

fn assert_projection(
    snapshot: &ProjectileAmmoSnapshot<'_>,
    catalog: &Value,
    magazine: u32,
    ammo: [u32; 2],
    encrypted: bool,
) -> TestResult {
    let [owner] = snapshot.rows() else {
        return Err("one selected physical weapon Item".into());
    };
    assert_selected(owner, magazine, ammo, encrypted)?;
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty()
            && owner.dependency_coverage.unreadable_bodies.is_empty(),
        "the unrelated unreadable Ammo body never enters either scope"
    );
    let scoped = json!({
        "projections": {"weaponProjectileAmmoSelections": {"rows": owner.selections}},
        "definitions": {"ammoDefinitions": {"rows": owner.ammo_definitions}},
        "provenance": snapshot.source_inventory(),
    });
    for path in [ROWS, AMMO] {
        assert_eq!(
            proof::rows(&scoped, path)?,
            proof::rows(catalog, path)?,
            "original values and source references agree at {path}"
        );
    }
    proof::closure(&scoped)
}

fn assert_selected(
    owner: &ProjectileAmmoOwner<'_>,
    magazine: u32,
    ammo: [u32; 2],
    encrypted: bool,
) -> TestResult {
    let [primary, secondary] = owner.selections.as_slice() else {
        return Err("both original launcher action occurrences".into());
    };
    let [primary_ammo, secondary_ammo] = owner.ammo_definitions.as_slice() else {
        return Err("both exact reached Ammo bodies".into());
    };
    let [primary_index, secondary_index] = ammo;
    for (row, role, definition, index) in [
        (
            primary,
            ProjectileAmmoRole::Primary,
            primary_ammo,
            primary_index,
        ),
        (
            secondary,
            ProjectileAmmoRole::Secondary,
            secondary_ammo,
            secondary_index,
        ),
    ] {
        let Some(ProjectileAmmoChoice::SelectedLinkedMagazine {
            role: actual_role,
            magazine_record_index,
            ammo_record_index,
            ..
        }) = row.selection().value()
        else {
            return Err("a uniquely linked external magazine".into());
        };
        assert_eq!(
            (
                actual_role.value(),
                magazine_record_index.value(),
                ammo_record_index.value()
            ),
            (Some(&role), Some(&magazine), Some(&index)),
            "the exact physical magazine and slot-selected Ammo"
        );
        assert_eq!(
            (
                definition.selection(),
                definition.source().record_index().value()
            ),
            (AmmoSelection::Dependency, Some(&index)),
            "the canonical external Ammo definition belongs to the same physical Record"
        );
        assert_eq!(
            row.selection().origin().contains_encrypted_source(),
            Some(encrypted),
            "the route evidence includes the original encryption marker"
        );
    }
    Ok(())
}

#[test]
fn ambiguous_magazine_headers_do_not_require_either_candidate_body() -> TestResult {
    let mut bytes = fixture::build(fixture::Case::DuplicateGuid).build();
    let forge = DataForge::open(&bytes)?;
    let weapon = physical(&forge, fixture::WEAPON)?;
    let guid = common::guid(fixture::MAGAZINE).parse()?;
    let candidates: Vec<_> = forge
        .records_by_guid(guid)
        .map(|record| record.index())
        .collect();
    assert_eq!(
        candidates.len(),
        2,
        "the ambiguous GUID names two physical headers"
    );
    for index in candidates {
        super::super::resolution::break_instance(&mut bytes, index)?;
    }
    let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Encrypted)?;
    with_projectile_ammo(&archive, weapon, None, |snapshot| -> TestResult {
        let [owner] = snapshot.rows() else {
            return Err("one selected physical weapon Item".into());
        };
        assert_eq!(owner.selections.len(), 2, "both original action outcomes");
        assert!(
            owner.selections.iter().all(|row| matches!(
                row.selection().value(),
                Some(ProjectileAmmoChoice::AmbiguousMagazineReference)
            )),
            "neither ambiguous physical target is admitted"
        );
        assert!(
            owner.ammo_definitions.is_empty()
                && owner.dependency_coverage.unreadable_bodies.is_empty(),
            "rejected candidates contribute no Ammo body or required dependency failure"
        );
        Ok(())
    })?
}

#[test]
fn a_proved_route_retains_an_unreadable_ammo_dependency() -> TestResult {
    let mut bytes = fixture::build(fixture::Case::Unique).build();
    let forge = DataForge::open(&bytes)?;
    let weapon = physical(&forge, fixture::WEAPON)?;
    let secondary = physical(&forge, fixture::SECONDARY)?;
    super::super::resolution::break_instance(&mut bytes, secondary)?;
    let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    with_projectile_ammo_from(&archive, &entry, weapon, None, |snapshot| -> TestResult {
        let [owner] = snapshot.rows() else {
            return Err("one selected physical weapon Item".into());
        };
        assert_eq!(
            owner.dependency_coverage.unreadable_bodies,
            [secondary],
            "the reached physical body failure survives the query"
        );
        assert_eq!(
            owner.ammo_definitions.len(),
            1,
            "the readable Ammo is retained"
        );
        assert!(
            owner.selections.iter().any(|row| matches!(
                row.selection().value(),
                Some(ProjectileAmmoChoice::SelectedLinkedMagazine { ammo_record_index, .. })
                    if ammo_record_index.value() == Some(&secondary)
            )),
            "the unreadable Ammo body has a proved route and physical target header"
        );
        Ok(())
    })?
}

fn physical(forge: &DataForge<'_>, guid: u128) -> TestResult<u32> {
    forge
        .record_by_guid(common::guid(guid).parse()?)
        .map(|record| record.index())
        .ok_or_else(|| "expected original physical Record".into())
}
