// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::{
    CapacityStatus, CargoGridOwner, CargoGridSnapshot, with_cargo_grids, with_cargo_grids_from,
};
use p4kx_extraction::{DataForgeEntry, SourceValueState};
use p4kx_testkit::{
    TestCargoGridLayout, TestRecord, cargo_grid_facility_database, cargo_grid_facility_database_for,
};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

const GRID: u32 = 1;
const CONTAINER: u32 = 2;
const UNRELATED: u32 = 4;

#[test]
fn one_cargo_grid_owner_matches_its_canonical_configuration_and_inventory_rows() -> TestResult {
    let database = cargo_grid_facility_database();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_directory, archive) = common::open(&database, origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        let decoded = DataForgeEntry::read(&archive)?;
        full_proof::from_entry(&archive, &decoded, full_proof::NO_LANGUAGES, None)?
            .with_cargo_grids(GRID, None, |snapshot| {
                verify_owner(&snapshot, &catalog, encrypted)
            })??;
    }
    Ok(())
}

#[test]
fn a_reached_inventory_failure_does_not_open_an_unrelated_unreadable_owner() -> TestResult {
    let database = cargo_grid_facility_database_for(TestCargoGridLayout::UnreadableStorage).record(
        TestRecord::new(
            0xf1166,
            p4kx_assembly::ENTITY_TYPE,
            "EntityClassDefinition.FixtureUnrelatedItem",
            "libs/foundry/records/entities/scitem/fixture/unrelated_item.xml",
        )
        .with_instance_index(u16::MAX),
    );
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let decoded = DataForgeEntry::read(&archive)?;
    let forge = decoded.open()?;
    for index in [CONTAINER, UNRELATED] {
        assert!(
            forge
                .record(index)
                .ok_or("original physical fixture Record")?
                .tree()
                .is_err(),
            "the fixture's nominated original instance is unreadable"
        );
    }
    with_cargo_grids_from(&archive, &decoded, GRID, None, |snapshot| -> TestResult {
        assert!(
            snapshot.coverage().unreadable_bodies.is_empty(),
            "the selected owner remains readable"
        );
        let owner = only_owner(&snapshot)?;
        assert_eq!(
            owner.dependency_coverage.unreadable_bodies,
            [CONTAINER],
            "only the physically reached inventory Record supplies a dependency failure"
        );
        assert!(
            owner.inventory_container.is_none(),
            "unreadable dependency supplies no definition row"
        );
        let capacity = only_grid(owner)?;
        assert!(
            matches!(capacity.status(), CapacityStatus::ContainerUnavailable),
            "the retained Grid reports its actual unavailable dependency"
        );
        assert_eq!(
            capacity.scu().state(),
            SourceValueState::Unreadable,
            "failed target is distinct from an absent operand"
        );
        Ok(())
    })?
}

#[test]
fn competing_original_storage_slots_never_select_an_inventory_definition() -> TestResult {
    let database = cargo_grid_facility_database_for(TestCargoGridLayout::DuplicateStorage);
    let (_directory, archive) = common::open(&database, DcbOrigin::Plain)?;
    with_cargo_grids(&archive, GRID, None, |snapshot| -> TestResult {
        let owner = only_owner(&snapshot)?;
        assert_eq!(
            owner.inventory_configurations.len(),
            2,
            "both original competing storage components are retained"
        );
        assert!(
            owner.inventory_container.is_none(),
            "competition leaves both targets outside the dependency read"
        );
        assert!(
            owner.dependency_coverage.unreadable_bodies.is_empty(),
            "unselected targets produce no body-read failure"
        );
        let capacity = only_grid(owner)?;
        assert!(
            matches!(capacity.status(), CapacityStatus::AmbiguousComponents),
            "canonical capacity reports ambiguous co-ownership"
        );
        assert_eq!(
            capacity.scu().state(),
            SourceValueState::Invalid,
            "ambiguous binding has no capacity value"
        );
        Ok(())
    })?
}

fn verify_owner(snapshot: &CargoGridSnapshot<'_>, catalog: &Value, encrypted: bool) -> TestResult {
    let owner = only_owner(snapshot)?;
    let capacity = only_grid(owner)?;
    assert!(
        matches!(capacity.status(), CapacityStatus::Available),
        "the original Open storage is available"
    );
    assert_eq!(
        capacity.scu().value(),
        Some(&2.0),
        "capacity uses the original inventory dimensions"
    );
    assert_eq!(
        capacity.scu().origin().contains_encrypted_source(),
        Some(encrypted),
        "the capacity marks the exact original Archive contribution"
    );
    let document = json!({
        "configurations": {
            "facilities": { "rows": owner.facilities },
            "inventoryConfigurations": { "rows": owner.inventory_configurations },
        },
        "definitions": {
            "inventoryContainers": { "rows": owner.inventory_container.as_slice() },
        },
        "provenance": snapshot.source_inventory(),
    });
    for path in [
        "/configurations/facilities/rows",
        "/configurations/inventoryConfigurations/rows",
        "/definitions/inventoryContainers/rows",
    ] {
        assert_eq!(
            proof::at(&document, path)?,
            proof::at(catalog, path)?,
            "SDK and Catalog preserve the same original values and contributor references at {path}"
        );
    }
    proof::closure(&document)
}

fn only_owner<'rows, 'source>(
    snapshot: &'rows CargoGridSnapshot<'source>,
) -> TestResult<&'rows CargoGridOwner<'source>> {
    let [owner] = snapshot.rows() else {
        return Err("one selected physical Cargo Grid owner".into());
    };
    let [grid] = owner.facilities.as_slice() else {
        return Err("one original Cargo Grid configuration".into());
    };
    let physical_owner = grid.owner().record_index().value();
    assert_eq!(
        physical_owner,
        Some(&GRID),
        "the configuration has the requested original owner"
    );
    Ok(owner)
}

fn only_grid<'rows>(
    owner: &'rows CargoGridOwner<'_>,
) -> TestResult<&'rows p4kx_export::catalog_queries::equipment::CargoGridCapacity> {
    let [row] = owner.facilities.as_slice() else {
        return Err("one original Cargo Grid configuration".into());
    };
    row.cargo_grid_fields()
        .and_then(|grid| grid.capacity())
        .ok_or_else(|| "finalized original Cargo Grid capacity".into())
}
