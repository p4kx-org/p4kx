// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestCapacity, TestEntity, TestLoadout, TestLoadoutEntry};

pub(super) const INVENTORY: u128 = 0x7a12;
pub(super) const GRID: u128 = 0x7a13;
const UNUSED_INVENTORY: u128 = 0x7a14;
const UNSIZED_GRID: u128 = 0x7a15;
pub(super) const SHIP: u128 = 0x7a16;
pub(super) const GRID_RECORD: u32 = 2;

pub(super) fn database(first: i32, second: i32, add_unresolved_capacity: bool) -> DataForgeBuilder {
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

pub(super) fn item(guid: u128, class_name: &str) -> TestEntity {
    TestEntity::new(
        guid,
        class_name,
        format!("libs/foundry/records/entities/scitem/ships/{class_name}.xml"),
    )
    .attach_type("CargoGrid")
}
