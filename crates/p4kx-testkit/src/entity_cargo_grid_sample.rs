// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    DataForgeBuilder, TestEntity, TestInstance, TestLoadout, TestLoadoutEntry, TestPort,
    TestRecord, TestStruct, TestType, TestValue, entity_schema, open_container_record,
};

const SHIP: u128 = 0x1063_0400;
const GRID: u128 = 0x1063_0401;
const STORAGE: u128 = 0x1063_0402;
const DAMAGE: u128 = 0x1063_0403;
const PORTS: [&str; 2] = ["grid_left", "grid_right"];

const GRID_TYPE: &str = "SCItemCargoGridParams";
const DAMAGE_TYPE: &str = "CommodityDamageConfiguration";

/// Creates two placements of one cargo grid and its co-located storage component.
#[must_use]
pub fn cargo_grid_facility_database() -> DataForgeBuilder {
    let mut database = entity_schema(DataForgeBuilder::new().version(8));
    database.structs.retain(|definition| {
        !matches!(
            definition.name.as_str(),
            "InventoryContainer" | "InventoryOpenContainerType"
        )
    });
    database
        .structure(grid_definition())
        .structure(TestStruct::new(DAMAGE_TYPE).field("minimumSpeed", TestType::Single))
        .structure(TestStruct::new("InventoryContainerTypeBase"))
        .structure(
            TestStruct::new("InventoryOpenContainerType")
                .parent("InventoryContainerTypeBase")
                .field("isExternalContainer", TestType::Bool),
        )
        .structure(
            TestStruct::new("InventoryContainer")
                .field("interiorDimensions", TestType::Class("Vec3".to_owned()))
                .field(
                    "inventoryType",
                    TestType::StrongPointer("InventoryContainerTypeBase".to_owned()),
                ),
        )
        .record(ship())
        .record(
            TestEntity::new(
                GRID,
                "FixtureCargoGrid",
                "libs/foundry/records/entities/scitem/fixture/cargo_grid.xml",
            )
            .attach_type("CargoGrid")
            .container(STORAGE)
            .component(grid_component())
            .record(),
        )
        .record(open_container_record(
            STORAGE,
            "FixtureCargoStorage",
            [1.25, 1.25, 2.5],
        ))
        .record(TestRecord::new(
            DAMAGE,
            DAMAGE_TYPE,
            "CommodityDamageConfiguration.Fixture",
            "libs/foundry/records/commoditydamageconfiguration/fixture.xml",
        ))
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

fn grid_component() -> TestValue {
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
            .value("invisible", TestValue::Bool(true))
            .value("miningOnly", TestValue::Bool(false))
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
