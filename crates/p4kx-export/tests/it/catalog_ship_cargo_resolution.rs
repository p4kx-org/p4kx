// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment;
use p4kx_export::{CatalogPublicationMode, ShipBaseOptions};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{DataForgeBuilder, TestCapacity, TestEntity, TestLoadout, TestLoadoutEntry};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

const INVENTORY: u128 = 0x7a12;
const GRID: u128 = 0x7a13;
const UNUSED_INVENTORY: u128 = 0x7a14;
const UNSIZED_GRID: u128 = 0x7a15;
const SHIP: u128 = 0x7a16;
const GRID_RECORD: u32 = 2;

#[test]
fn ship_cargo_uses_the_first_physical_duplicate_for_each_mount() -> TestResult {
    for (first, second, expected) in [(125, 975, 2.5), (975, 125, 19.5)] {
        let database = database(first, second, false);
        let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
        let entry = DataForgeEntry::read(&archive)?;
        assert_inventory_order(&entry)?;
        assert_cargo(&archive, &entry, Some(expected), 0)?;
        let scoped_has_facility =
            equipment::with_cargo_grids_from(&archive, &entry, GRID_RECORD, None, |snapshot| {
                !snapshot.rows().is_empty()
            })?;
        assert!(
            !scoped_has_facility,
            "the attachable grid needs no independent facility component for ship aggregation"
        );
    }
    Ok(())
}

#[test]
fn ship_cargo_keeps_the_lower_bound_when_a_mounted_inventory_reference_is_dangling() -> TestResult {
    let database = database(125, 975, true);
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    assert_cargo(&archive, &entry, Some(2.5), 1)
}

#[test]
fn ship_cargo_keeps_the_first_invalid_duplicate_and_reports_unknown_capacity() -> TestResult {
    let database = database(-125, 975, false);
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    assert_cargo(&archive, &entry, None, 2)
}

fn database(first: i32, second: i32, add_unresolved_capacity: bool) -> DataForgeBuilder {
    let grid = item(GRID, "Fixture_WordGrid").container(INVENTORY);
    let mut builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .record(p4kx_testkit::closed_container_record(
            INVENTORY,
            "fixture_first",
            TestCapacity::Centi(first),
        ))
        .record(p4kx_testkit::closed_container_record(
            INVENTORY,
            "fixture_second",
            TestCapacity::Centi(second),
        ))
        .record(grid.record());
    let mut entries = vec![
        TestLoadoutEntry::class("grid_left", "Fixture_WordGrid"),
        TestLoadoutEntry::class("grid_right", "Fixture_WordGrid"),
    ];
    if add_unresolved_capacity {
        let grid = item(UNSIZED_GRID, "Fixture_UnsizedGrid").container(UNUSED_INVENTORY);
        builder = builder.record(grid.record());
        entries.push(TestLoadoutEntry::class(
            "grid_unknown",
            "Fixture_UnsizedGrid",
        ));
    }
    builder.record(
        TestEntity::new(
            SHIP,
            "Fixture_CargoShip",
            "libs/foundry/records/entities/spaceships/fixture_cargo_ship.xml",
        )
        .loadout(TestLoadout::Manual(entries))
        .record(),
    )
}

fn item(guid: u128, class_name: &str) -> TestEntity {
    TestEntity::new(
        guid,
        class_name,
        format!("libs/foundry/records/entities/scitem/ships/{class_name}.xml"),
    )
    .attach_type("CargoGrid")
}

fn assert_inventory_order(entry: &DataForgeEntry) -> TestResult {
    let forge = entry.open()?;
    let first = forge.record(0).ok_or("first original inventory Record")?;
    let second = forge.record(1).ok_or("second original inventory Record")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "the same authored inventory GUID"
    );
    assert_ne!(
        first.record_path()?,
        second.record_path()?,
        "two distinct physical definition paths"
    );
    assert_eq!(
        forge
            .records_by_guid(first.guid())
            .map(|record| record.index())
            .collect::<Vec<_>>(),
        [0, 1],
        "both physical aliases survive in stored order"
    );
    assert_eq!(
        forge
            .record_by_guid(first.guid())
            .ok_or("selected original inventory Record")?
            .index(),
        first.index(),
        "first physical definition is the existing aggregate's lookup"
    );
    Ok(())
}

fn assert_cargo(
    archive: &Archive,
    entry: &DataForgeEntry,
    expected_scu: Option<f64>,
    expected_issues: usize,
) -> TestResult {
    let ship_base =
        p4kx_export::build_ship_base_from(archive, entry, &ShipBaseOptions::new(UNIX_EPOCH))?;
    let [base] = ship_base.ships() else {
        return Err("one selected synthetic ship".into());
    };
    assert_eq!(
        base.cargo.scu, expected_scu,
        "direct Ship Base cargo uses every reached mount and the first physical definition"
    );
    assert_eq!(
        base.cargo.grids_with_issue, expected_issues,
        "one issue per reached mount without a usable capacity"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog = p4kx_export::build_catalog_from(archive, entry, &options)?;
    let json: Value = serde_json::from_slice(catalog.json())?;
    let [row] = json
        .pointer("/ships")
        .and_then(Value::as_array)
        .ok_or("canonical ships list")?
        .as_slice()
    else {
        return Err("one original ship in the canonical projection".into());
    };
    assert_eq!(
        row["cargoScu"],
        json!(expected_scu),
        "canonical quantity agrees with Ship Base"
    );
    assert_eq!(
        row["cargoGridsWithIssue"],
        json!(expected_issues),
        "canonical issue count agrees with Ship Base"
    );
    Ok(())
}
