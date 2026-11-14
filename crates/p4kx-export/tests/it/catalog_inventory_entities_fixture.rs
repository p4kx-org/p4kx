// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue};

use super::fixture::{self, class, pointer, vec3};

pub(super) const WEARABLE: u128 = 0x1057_0020;
pub(super) const CARGO_ITEM: u128 = 0x1057_0021;
pub(super) const VEHICLE: u128 = 0x1057_0022;
pub(super) const MISSING: u128 = 0x1057_0030;
pub(super) const DIMENSION_BITS: u32 = 0x7fc0_1057;

const ATTACHABLE: &str = "SAttachableComponentParams";
const ITEM: &str = "SItemDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const STORAGE: &str = "SCItemInventoryContainerComponentParams";
const VEHICLE_COMPONENT: &str = "VehicleComponentParams";
const OBJECT_METADATA: &str = "SEntityComponentObjectMetadataParams";
const INTERACTION: &str = "SSharedInteractionParams";
const CONTAINER: &str = "InventoryContainer";

pub(super) fn database() -> DataForgeBuilder {
    definitions(fixture::database(), item_definition())
        .record(
            TestEntity::new(
                WEARABLE,
                "FixtureWearable",
                "libs/foundry/records/entities/scitem/fixture/wearable.xml",
            )
            .component(wearable())
            .component(metadata(false))
            .record(),
        )
        .record(
            TestEntity::new(
                CARGO_ITEM,
                "FixtureStorageItem",
                "libs/foundry/records/entities/scitem/fixture/storage_item.xml",
            )
            .component(storage(fixture::OPEN, true))
            .component(storage(MISSING, false))
            .record(),
        )
        .record(
            TestEntity::new(
                VEHICLE,
                "FixtureStorageVehicle",
                "libs/foundry/records/entities/spaceships/fixture/vehicle.xml",
            )
            .component(vehicle())
            .component(metadata(true))
            .record(),
        )
}

fn definitions(builder: DataForgeBuilder, item: TestStruct) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("EntityClassDefinition")
                .field("Category", TestType::String)
                .array("Components", pointer(COMPONENT)),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new("Vec2")
                .field("x", TestType::Single)
                .field("y", TestType::Single),
        )
        .structure(TestStruct::new(INTERACTION).field("fixtureInteractionMarker", TestType::Bool))
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", class(ITEM)),
        )
        .structure(item)
        .structure(
            TestStruct::new(STORAGE)
                .parent(COMPONENT)
                .field(
                    "containerParams",
                    TestType::ReferenceTo(CONTAINER.to_owned()),
                )
                .field(
                    "openInventoryInteraction",
                    TestType::WeakPointer(INTERACTION.to_owned()),
                )
                .field(
                    "openLootingInteraction",
                    TestType::WeakPointer(INTERACTION.to_owned()),
                )
                .field("canAutoStoreRestrictedItems", TestType::Bool)
                .field("fixtureInteraction", pointer(INTERACTION)),
        )
        .structure(
            TestStruct::new(VEHICLE_COMPONENT)
                .parent(COMPONENT)
                .field(
                    "inventoryContainerParams",
                    TestType::ReferenceTo(CONTAINER.to_owned()),
                )
                .field("maxBoundingBoxSize", class("Vec3")),
        )
        .structure(
            TestStruct::new(OBJECT_METADATA)
                .parent(COMPONENT)
                .field("hasLocalBounds", TestType::Bool),
        )
}

fn item_definition() -> TestStruct {
    TestStruct::new(ITEM)
        .field("Type", TestType::String)
        .field("inventoryOccupancyVolume", pointer("SBaseCargoUnit"))
        .field("inventoryOccupancyDimensions", class("Vec3"))
        .field("inventoryOccupancyLocalBoundsMin", class("Vec3"))
        .field("inventoryOccupancyLocalBoundsMax", class("Vec3"))
        .field("inventoryOccupancyDimensionsUIOverride", pointer("Vec3"))
        .field("inventoryOccupancyFixedGridDimensionsUIV4", class("Vec2"))
}

pub(super) fn wrong_dimension_type() -> DataForgeBuilder {
    let declaration = TestStruct::new(ITEM)
        .field("Type", TestType::String)
        .field("inventoryOccupancyDimensions", TestType::String)
        .field("inventoryOccupancyDimensionsUIOverride", pointer("Vec3"));
    let definition = TestInstance::new(ITEM)
        .value("Type", TestValue::text("Clothing"))
        .value(
            "inventoryOccupancyDimensions",
            TestValue::text("FixtureWrongType"),
        )
        .value(
            "inventoryOccupancyDimensionsUIOverride",
            vec3([4.0, -0.0, 2.0]),
        );
    let component = TestInstance::new(ATTACHABLE).value("AttachDef", TestValue::Struct(definition));
    definitions(fixture::database(), declaration).record(
        TestEntity::new(
            WEARABLE,
            "FixtureWearable",
            "libs/foundry/records/entities/scitem/fixture/wearable.xml",
        )
        .component(TestValue::Struct(component))
        .component(metadata(false))
        .record(),
    )
}

fn wearable() -> TestValue {
    TestValue::Struct(
        TestInstance::new(ATTACHABLE).value(
            "AttachDef",
            TestValue::Struct(
                TestInstance::new(ITEM)
                    .value("Type", TestValue::text("Clothing"))
                    .value(
                        "inventoryOccupancyVolume",
                        TestValue::Struct(
                            TestInstance::new("SMicroCargoUnit")
                                .value("microSCU", TestValue::Int32(0)),
                        ),
                    )
                    .value(
                        "inventoryOccupancyDimensions",
                        vec3([0.0, -0.0, f32::from_bits(DIMENSION_BITS)]),
                    )
                    .value("inventoryOccupancyLocalBoundsMin", vec3([-1.0, -2.0, -3.0]))
                    .value("inventoryOccupancyLocalBoundsMax", vec3([1.0, 2.0, 3.0]))
                    .value("inventoryOccupancyDimensionsUIOverride", TestValue::Null)
                    .value(
                        "inventoryOccupancyFixedGridDimensionsUIV4",
                        TestValue::Struct(
                            TestInstance::new("Vec2")
                                .value("x", TestValue::Single(4.0))
                                .value("y", TestValue::Single(0.0)),
                        ),
                    ),
            ),
        ),
    )
}

fn storage(target: u128, present: bool) -> TestValue {
    let mut row = TestInstance::new(STORAGE)
        .value("containerParams", TestValue::Reference(target))
        .value("canAutoStoreRestrictedItems", TestValue::Bool(false));
    if present {
        row = row
            .value(
                "fixtureInteraction",
                TestValue::Struct(TestInstance::new(INTERACTION).named("fixture-open")),
            )
            .value(
                "openInventoryInteraction",
                TestValue::pointer("fixture-open"),
            );
    }
    TestValue::Struct(row)
}

fn metadata(value: bool) -> TestValue {
    TestValue::Struct(
        TestInstance::new(OBJECT_METADATA).value("hasLocalBounds", TestValue::Bool(value)),
    )
}

fn vehicle() -> TestValue {
    TestValue::Struct(
        TestInstance::new(VEHICLE_COMPONENT)
            .value(
                "inventoryContainerParams",
                TestValue::Reference(fixture::STANDARD),
            )
            .value("maxBoundingBoxSize", vec3([5.0, 6.0, 7.0])),
    )
}
