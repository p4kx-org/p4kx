// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_cargo;
use crate::{
    DataForgeBuilder, TestCapacity, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry,
    TestPort, TestRecord, TestStruct, TestType, TestValue, closed_container_record, entity_schema,
    open_container_record,
};

const SHIP: u128 = 0x1063_0400;
const GRID: u128 = 0x1063_0401;
const STORAGE: u128 = 0x1063_0402;
const DAMAGE: u128 = 0x1063_0403;
const SECOND_STORAGE: u128 = 0x1063_0404;
const PORTS: [&str; 2] = ["grid_left", "grid_right"];

const GRID_TYPE: &str = "SCItemCargoGridParams";
const DAMAGE_TYPE: &str = "CommodityDamageConfiguration";
const INVENTORY_TYPE: &str = "InventoryContainerTypeBase";
const CAPACITY_TYPE: &str = "SBaseCargoUnit";
const CELL_METRES: f32 = 1.25;

/// The authored grid flags and co-owned storage in a cargo-grid fixture.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum TestCargoGridLayout {
    /// An invisible, non-mining grid with a unique open container.
    Invisible,
    /// A visible mining-only grid with a unique open container.
    MiningOnly,
    /// A grid without a co-owned storage component.
    MissingStorage,
    /// A grid with two storage components pointing to distinct containers.
    DuplicateStorage,
    /// A grid whose resolved storage holds the unspecialized inventory type.
    UnsupportedStorage,
    /// A grid whose resolved closed container stores the chosen unit.
    Closed(TestCapacity),
}

/// Creates two placements of one cargo grid and its co-located storage component.
#[must_use]
pub fn cargo_grid_facility_database() -> DataForgeBuilder {
    cargo_grid_facility_database_for(TestCargoGridLayout::Invisible)
}

/// Creates two placements of one cargo grid with the given authored layout.
#[must_use]
pub fn cargo_grid_facility_database_for(layout: TestCargoGridLayout) -> DataForgeBuilder {
    let mut database = entity_schema(DataForgeBuilder::new().version(8));
    database.structs.retain(|definition| {
        !matches!(
            definition.name.as_str(),
            "InventoryContainer" | "InventoryOpenContainerType" | "InventoryClosedContainerType"
        )
    });
    for definition in &mut database.structs {
        if definition.parent.as_deref() == Some("InventoryCapacityBase") {
            definition.parent = Some(CAPACITY_TYPE.to_owned());
        }
    }
    let database = database
        .structure(grid_definition())
        .structure(TestStruct::new(DAMAGE_TYPE).field("minimumSpeed", TestType::Single))
        .structure(TestStruct::new(INVENTORY_TYPE))
        .structure(TestStruct::new(CAPACITY_TYPE))
        .structure(
            TestStruct::new("InventoryClosedContainerType")
                .parent(INVENTORY_TYPE)
                .field(
                    "capacity",
                    TestType::StrongPointer(CAPACITY_TYPE.to_owned()),
                ),
        )
        .structure(
            TestStruct::new("InventoryOpenContainerType")
                .parent(INVENTORY_TYPE)
                .field("isExternalContainer", TestType::Bool),
        )
        .structure(
            TestStruct::new("InventoryContainer")
                .field("interiorDimensions", TestType::Class("Vec3".to_owned()))
                .field(
                    "inventoryType",
                    TestType::StrongPointer(INVENTORY_TYPE.to_owned()),
                ),
        )
        .record(ship())
        .record(grid(layout))
        .record(storage(layout))
        .record(TestRecord::new(
            DAMAGE,
            DAMAGE_TYPE,
            "CommodityDamageConfiguration.Fixture",
            "libs/foundry/records/commoditydamageconfiguration/fixture.xml",
        ));
    if layout == TestCargoGridLayout::DuplicateStorage {
        return database.record(open_container_record(
            SECOND_STORAGE,
            "FixtureCompetingCargoStorage",
            [CELL_METRES, CELL_METRES * 2.0, CELL_METRES * 2.0],
        ));
    }
    database
}

fn grid(layout: TestCargoGridLayout) -> TestRecord {
    let mut grid = TestEntity::new(
        GRID,
        "FixtureCargoGrid",
        "libs/foundry/records/entities/scitem/fixture/cargo_grid.xml",
    )
    .attach_type("CargoGrid");
    if layout != TestCargoGridLayout::MissingStorage {
        grid = grid.container(STORAGE);
    }
    grid = grid.component(grid_component(layout));
    if layout == TestCargoGridLayout::DuplicateStorage {
        grid = grid.component(entity_cargo::container_component(SECOND_STORAGE));
    }
    grid.record()
}

fn storage(layout: TestCargoGridLayout) -> TestRecord {
    if let TestCargoGridLayout::Closed(capacity) = layout {
        return closed_container_record(STORAGE, "FixtureCargoStorage", capacity);
    }
    let mut record = open_container_record(
        STORAGE,
        "FixtureCargoStorage",
        [CELL_METRES, CELL_METRES, CELL_METRES * 2.0],
    );
    if layout == TestCargoGridLayout::UnsupportedStorage {
        record.values.retain(|(name, _)| name != "inventoryType");
        return record.value(
            "inventoryType",
            TestValue::Struct(TestInstance::new(INVENTORY_TYPE)),
        );
    }
    record
}

fn grid_definition() -> TestStruct {
    TestStruct::new(GRID_TYPE)
        .parent("DataForgeComponentParams")
        .field("dimensions", TestType::Class("Vec3".to_owned()))
        .field("crateGenPercentageOnDestroy", TestType::Single)
        .field("crateMaxOnDestroy", TestType::Int32)
        .field("invisible", TestType::Bool)
        .field("miningOnly", TestType::Bool)
        .field("transformDependant", TestType::Bool)
        .field("minVolatilePowerToExplode", TestType::Single)
        .field(
            "damageConfiguration",
            TestType::ReferenceTo(DAMAGE_TYPE.to_owned()),
        )
        .field("fixtureExtra", TestType::UInt16)
}

fn grid_component(layout: TestCargoGridLayout) -> TestValue {
    let mining_only = layout == TestCargoGridLayout::MiningOnly;
    let dimensions = TestInstance::new("Vec3")
        .value("x", TestValue::Single(1_250.0))
        .value("y", TestValue::Single(-0.0))
        .value("z", TestValue::Single(f32::INFINITY));
    TestValue::Struct(
        TestInstance::new(GRID_TYPE)
            .value("dimensions", TestValue::Struct(dimensions))
            .value(
                "crateGenPercentageOnDestroy",
                TestValue::Single(f32::from_bits(0x7fc0_0042)),
            )
            .value("crateMaxOnDestroy", TestValue::Int32(i32::MAX))
            .value("invisible", TestValue::Bool(!mining_only))
            .value("miningOnly", TestValue::Bool(mining_only))
            .value("transformDependant", TestValue::Bool(true))
            .value("minVolatilePowerToExplode", TestValue::Single(-0.0))
            .value("damageConfiguration", TestValue::Reference(DAMAGE))
            .value("fixtureExtra", TestValue::UInt16(14)),
    )
}

fn ship() -> TestRecord {
    let mounts = PORTS
        .map(|port| TestLoadoutEntry::reference(port, GRID))
        .to_vec();
    PORTS
        .into_iter()
        .fold(
            TestEntity::new(
                SHIP,
                "FixtureCargoHost",
                "libs/foundry/records/entities/spaceships/fixture_cargo_host.xml",
            )
            .vehicle("", "")
            .loadout(TestLoadout::Manual(mounts)),
            |ship, port| ship.port(TestPort::new(port)),
        )
        .record()
}
