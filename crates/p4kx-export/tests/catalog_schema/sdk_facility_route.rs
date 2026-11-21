// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{
    CapacityStatus, FacilityKind, FacilityOwner, FacilitySnapshot, with_facilities_from,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, cargo_grid_facility_database, test_key,
};
use serde_json::Value;

use super::super::FACILITIES;
use super::{Route, TestResult, facility_medbed_fixture as medical_bed};

#[path = "../it/catalog_facility_capabilities_fixture.rs"]
mod capabilities;
#[path = "../it/catalog_facility_seat_fixture.rs"]
mod seat;

fn fixtures() -> [(DataForgeBuilder, u32, &'static [FacilityKind]); 5] {
    [
        (
            cargo_grid_facility_database(),
            1,
            &[FacilityKind::CargoGrid],
        ),
        (
            seat::database(1.0),
            0,
            &[FacilityKind::Seat, FacilityKind::Seat],
        ),
        (
            seat::database_with_alternative_ejection(None),
            0,
            &[FacilityKind::Seat],
        ),
        (medical_bed::database(), 0, &[FacilityKind::MedBed]),
        (
            capabilities::database(),
            0,
            &[
                FacilityKind::OutfitHanger,
                FacilityKind::Logout,
                FacilityKind::Logout,
            ],
        ),
    ]
}

pub(super) fn read() -> TestResult<Route> {
    let mut route = None;
    for (database, record, kinds) in fixtures() {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("facility.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored(r"Data\Game2.dcb", database.build()).encrypted())
            .write_to(&path)?;
        let archive = Archive::open_with_key(path, Some(test_key()))?;
        let entry = DataForgeEntry::read(&archive)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
        )?;
        if route.is_none() {
            verify_other_selections(&archive, &entry, &catalog, record, kinds)?;
        }
        with_facilities_from(
            &archive,
            &entry,
            CatalogRecordSelection::RecordIndex(record),
            None,
            |snapshot| -> TestResult {
                check(&snapshot, &catalog, record, kinds)?;
                if route.is_none() {
                    let [owner] = snapshot.rows() else {
                        return Err("the selected fixture facility owner must project".into());
                    };
                    route = Some(Route::queried_rows(
                        FACILITIES,
                        &owner.facilities,
                        "all authored Facility variants have synthetic producer parity; installed producer parity open",
                    )?);
                }
                Ok(())
            },
        )??;
    }
    route.ok_or_else(|| "a canonical Facility witness is required".into())
}

fn verify_other_selections(
    archive: &Archive,
    entry: &DataForgeEntry,
    catalog: &Value,
    record: u32,
    kinds: &[FacilityKind],
) -> TestResult {
    let forge = entry.open()?;
    let owner = forge
        .record(record)
        .ok_or("the physical facility fixture")?;
    for selection in [
        CatalogRecordSelection::Guid(owner.guid()),
        CatalogRecordSelection::All,
    ] {
        with_facilities_from(archive, entry, selection, None, |snapshot| {
            check(&snapshot, catalog, record, kinds)
        })??;
    }
    Ok(())
}

fn check(
    snapshot: &FacilitySnapshot<'_>,
    catalog: &Value,
    record: u32,
    kinds: &[FacilityKind],
) -> TestResult {
    let [owner] = snapshot.rows() else {
        return Err("one selected physical facility owner".into());
    };
    let actual = serde_json::to_value(&owner.facilities)?;
    let expected = catalog
        .pointer("/configurations/facilities/rows")
        .ok_or("Catalog Facility rows")?;
    assert_eq!(
        &actual, expected,
        "original facility rows and their provenance refs match"
    );
    for (row, kind) in owner.facilities.iter().zip(kinds) {
        assert_eq!(
            row.owner().record_index().value(),
            Some(&record),
            "the projected row belongs to the selected physical owner"
        );
        assert_eq!(
            row.source_type().origin().contains_encrypted_source(),
            Some(true),
            "facility source type retains DataForge encryption"
        );
        assert_eq!(
            row.kind(),
            *kind,
            "the typed scope retains original component order and family"
        );
    }
    assert_eq!(
        owner.facilities.len(),
        kinds.len(),
        "every authored facility occurrence was retained"
    );
    assert_eq!(
        snapshot.coverage().unreadable_bodies,
        [] as [u32; 0],
        "the selected fixture owner was readable"
    );
    assert_eq!(
        owner.dependency_coverage.unreadable_bodies,
        [] as [u32; 0],
        "the fixture has no unreadable admitted storage dependency"
    );
    check_capacity(owner, catalog, kinds.contains(&FacilityKind::CargoGrid))
}

fn check_capacity(owner: &FacilityOwner<'_>, catalog: &Value, has_grid: bool) -> TestResult {
    if has_grid {
        let capacity = owner
            .facilities
            .iter()
            .find_map(|facility| {
                facility
                    .cargo_grid_fields()
                    .and_then(|grid| grid.capacity())
            })
            .ok_or("finalized canonical Cargo Grid capacity")?;
        assert!(
            matches!(capacity.status(), CapacityStatus::Available),
            "the original grid storage is reachable"
        );
        assert_eq!(
            capacity.scu().value(),
            Some(&2.0),
            "capacity uses the original inventory dimensions"
        );
        assert_eq!(
            capacity.scu().origin().contains_encrypted_source(),
            Some(true),
            "capacity retains encrypted operand evidence"
        );
        assert_eq!(
            serde_json::to_value(owner.inventory_container.as_slice())?,
            *catalog
                .pointer("/definitions/inventoryContainers/rows")
                .ok_or("linked inventory row")?,
            "the query preserves the original capacity dependency and its origin references"
        );
    } else {
        let ([], None) = (
            owner.inventory_configurations.as_slice(),
            &owner.inventory_container,
        ) else {
            return Err(
                "a facility owner without a Grid must have no capacity dependencies".into(),
            );
        };
    }
    Ok(())
}
