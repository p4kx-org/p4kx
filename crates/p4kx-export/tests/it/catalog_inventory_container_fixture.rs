// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OPEN: u128 = 0x1057_0010;
pub(super) const STANDARD: u128 = 0x1057_0011;
pub(super) const CENTI: u128 = 0x1057_0012;
pub(super) const MICRO: u128 = 0x1057_0013;
pub(super) const FUTURE: u128 = 0x1057_0014;
pub(super) const METER_GRID: u128 = 0x1057_0015;

const CONTAINER: &str = "InventoryContainer";
const TYPE_BASE: &str = "InventoryContainerTypeBase";
const OPEN_TYPE: &str = "InventoryOpenContainerType";
const CLOSED_TYPE: &str = "InventoryClosedContainerType";
const FILTER: &str = "InventoryContainerItemTypeFilter";
const GRID_BASE: &str = "InventoryContainerGridCellSizeBase";
const GRID_CM: &str = "InventoryContainerGridCellSizeCentimeters";
const GRID_M: &str = "InventoryContainerGridCellSizeMeters";
const CARGO_BASE: &str = "SBaseCargoUnit";

pub(super) fn database() -> DataForgeBuilder {
    let builder = units(container_schema(DataForgeBuilder::new().version(8)));
    builder
        .record(record(
            OPEN,
            "fixture_open",
            open(TestInstance::new(GRID_CM).value("centimeters", TestValue::Int32(0))),
        ))
        .record(record(
            METER_GRID,
            "fixture_meters",
            open(TestInstance::new(GRID_M).value("meters", TestValue::Int32(3))),
        ))
        .record(record(
            STANDARD,
            "fixture_standard",
            closed(
                "SStandardCargoUnit",
                "standardCargoUnits",
                TestValue::Single(2.75),
            ),
        ))
        .record(record(
            CENTI,
            "fixture_centi",
            closed("SCentiCargoUnit", "centiSCU", TestValue::Int32(-125)),
        ))
        .record(record(
            MICRO,
            "fixture_micro",
            closed("SMicroCargoUnit", "microSCU", TestValue::Int32(0)),
        ))
        .record(record(
            FUTURE,
            "fixture_future",
            closed("FixtureCargoUnit", "fixtureQuanta", TestValue::UInt16(19)),
        ))
}

fn container_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration("EItemType", ["FixtureUtility"])
        .enumeration("EItemSubType", ["FixtureSmall", "FixtureMedium"])
        .structure(
            TestStruct::new("Vec3")
                .field("x", TestType::Single)
                .field("y", TestType::Single)
                .field("z", TestType::Single),
        )
        .structure(
            TestStruct::new(CONTAINER)
                .array("excludedItemSubTypes", class(FILTER))
                .field("interiorDimensions", class("Vec3"))
                .field("inventoryType", pointer(TYPE_BASE)),
        )
        .structure(
            TestStruct::new(FILTER)
                .field("itemType", TestType::Enum("EItemType".to_owned()))
                .array("itemSubTypes", TestType::Enum("EItemSubType".to_owned())),
        )
        .structure(TestStruct::new(TYPE_BASE))
        .structure(
            TestStruct::new(OPEN_TYPE)
                .parent(TYPE_BASE)
                .field("gridCellSize", pointer(GRID_BASE))
                .field("minPermittedItemSize", class("Vec3"))
                .field("maxPermittedItemSize", class("Vec3"))
                .field("isExternalContainer", TestType::Bool)
                .field("gridPosOffset", class("Vec3"))
                .field("maxPercentageErasedOnParentDestruction", TestType::Int32)
                .field("randomDestructionDistributionExponent", TestType::Single)
                .field("appliedForceOnParentDestruction", TestType::Single)
                .array("overrideStorableItemTypes", class(FILTER)),
        )
        .structure(
            TestStruct::new(CLOSED_TYPE)
                .parent(TYPE_BASE)
                .field("capacity", pointer(CARGO_BASE)),
        )
}

fn units(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(GRID_BASE))
        .structure(
            TestStruct::new(GRID_CM)
                .parent(GRID_BASE)
                .field("centimeters", TestType::Int32),
        )
        .structure(
            TestStruct::new(GRID_M)
                .parent(GRID_BASE)
                .field("meters", TestType::Int32),
        )
        .structure(TestStruct::new(CARGO_BASE))
        .structure(
            TestStruct::new("SStandardCargoUnit")
                .parent(CARGO_BASE)
                .field("standardCargoUnits", TestType::Single),
        )
        .structure(
            TestStruct::new("SCentiCargoUnit")
                .parent(CARGO_BASE)
                .field("centiSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new("SMicroCargoUnit")
                .parent(CARGO_BASE)
                .field("microSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new("FixtureCargoUnit")
                .parent(CARGO_BASE)
                .field("fixtureQuanta", TestType::UInt16),
        )
}

fn record(guid: u128, name: &str, storage: TestInstance) -> TestRecord {
    TestRecord::new(
        guid,
        CONTAINER,
        format!("InventoryContainer.{name}"),
        format!("libs/foundry/records/fixture_storage/{name}.xml"),
    )
    .value("excludedItemSubTypes", TestValue::Array(vec![filter()]))
    .value("interiorDimensions", vec3([0.0, -0.0, 4.25]))
    .value("inventoryType", TestValue::Struct(storage))
}

fn open(grid_size: TestInstance) -> TestInstance {
    TestInstance::new(OPEN_TYPE)
        .value("gridCellSize", TestValue::Struct(grid_size))
        .value("minPermittedItemSize", vec3([0.0, 1.0, 2.0]))
        .value("maxPermittedItemSize", vec3([3.0, 4.0, 5.0]))
        .value("isExternalContainer", TestValue::Bool(false))
        .value("gridPosOffset", vec3([-0.0, -1.0, 2.0]))
        .value(
            "maxPercentageErasedOnParentDestruction",
            TestValue::Int32(0),
        )
        .value(
            "randomDestructionDistributionExponent",
            TestValue::Single(1.25),
        )
        .value("appliedForceOnParentDestruction", TestValue::Single(-0.0))
        .value(
            "overrideStorableItemTypes",
            TestValue::Array(vec![filter(), filter()]),
        )
}

fn closed(unit: &str, field: &str, value: TestValue) -> TestInstance {
    TestInstance::new(CLOSED_TYPE).value(
        "capacity",
        TestValue::Struct(TestInstance::new(unit).value(field, value)),
    )
}

fn filter() -> TestValue {
    TestValue::Struct(
        TestInstance::new(FILTER)
            .value("itemType", TestValue::text("FixtureUtility"))
            .value(
                "itemSubTypes",
                TestValue::Array(
                    ["FixtureSmall", "FixtureMedium", "FixtureSmall"]
                        .map(TestValue::text)
                        .to_vec(),
                ),
            ),
    )
}

pub(super) fn vec3([x, y, z]: [f32; 3]) -> TestValue {
    TestValue::Struct(
        TestInstance::new("Vec3")
            .value("x", TestValue::Single(x))
            .value("y", TestValue::Single(y))
            .value("z", TestValue::Single(z)),
    )
}

pub(super) fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

pub(super) fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
