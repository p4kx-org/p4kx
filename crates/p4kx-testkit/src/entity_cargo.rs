// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestRecord, TestStruct, TestType, TestValue};

/// The capacity of a closed inventory container, in the unit the Record states it in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TestCapacity {
    /// `SStandardCargoUnit`: standard cargo units.
    Standard(f32),
    /// `SCentiCargoUnit`: hundredths of a standard cargo unit.
    Centi(i32),
    /// `SMicroCargoUnit`: millionths of a standard cargo unit, stored as a signed integer.
    Micro(i32),
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

pub(crate) fn cargo_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("SEntityPhysicsControllerParams")
            .parent("DataForgeComponentParams")
            .field("PhysType", pointer("SEntityPhysicsControllerBase")),
        TestStruct::new("SEntityPhysicsControllerBase"),
        TestStruct::new("SEntityRigidPhysicsControllerParams")
            .parent("SEntityPhysicsControllerBase")
            .field("Mass", TestType::Single),
        TestStruct::new("SCItemInventoryContainerComponentParams")
            .parent("DataForgeComponentParams")
            .field(
                "containerParams",
                TestType::ReferenceTo("InventoryContainer".to_owned()),
            ),
        TestStruct::new("InventoryContainer")
            .field("interiorDimensions", TestType::Class("Vec3".to_owned()))
            .field("inventoryType", pointer("InventoryTypeBase")),
        TestStruct::new("InventoryTypeBase"),
        TestStruct::new("InventoryOpenContainerType")
            .parent("InventoryTypeBase")
            .field("isExternalContainer", TestType::Bool),
        TestStruct::new("InventoryClosedContainerType")
            .parent("InventoryTypeBase")
            .field("capacity", pointer("InventoryCapacityBase")),
        TestStruct::new("InventoryCapacityBase"),
        TestStruct::new("SStandardCargoUnit")
            .parent("InventoryCapacityBase")
            .field("standardCargoUnits", TestType::Single),
        TestStruct::new("SCentiCargoUnit")
            .parent("InventoryCapacityBase")
            .field("centiSCU", TestType::Int32),
        TestStruct::new("SMicroCargoUnit")
            .parent("InventoryCapacityBase")
            .field("microSCU", TestType::Int32),
    ]
}

pub(crate) fn physics_component(mass: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SEntityPhysicsControllerParams").value(
            "PhysType",
            TestValue::Struct(
                TestInstance::new("SEntityRigidPhysicsControllerParams")
                    .value("Mass", TestValue::Single(mass)),
            ),
        ),
    )
}

pub(crate) fn container_component(container: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SCItemInventoryContainerComponentParams")
            .value("containerParams", TestValue::Reference(container)),
    )
}

fn container_record(guid: u128, name: &str, dimensions: [f32; 3], kind: TestValue) -> TestRecord {
    let [x, y, z] = dimensions;
    TestRecord::new(
        guid,
        "InventoryContainer",
        format!("InventoryContainer.{name}"),
        format!("libs/foundry/records/inventorycontainers/{name}.xml"),
    )
    .value(
        "interiorDimensions",
        TestValue::Struct(
            TestInstance::new("Vec3")
                .value("x", TestValue::Single(x))
                .value("y", TestValue::Single(y))
                .value("z", TestValue::Single(z)),
        ),
    )
    .value("inventoryType", kind)
}

/// Returns the `InventoryContainer` Record `name` of an open container, a cargo grid.
///
/// The interior is `dimensions` in metres; an item's `SCItemInventoryContainerComponentParams`
/// refers to the Record. The database needs [`entity_schema`](crate::entity_schema).
#[must_use]
pub fn open_container_record(guid: u128, name: &str, dimensions: [f32; 3]) -> TestRecord {
    container_record(
        guid,
        name,
        dimensions,
        TestValue::Struct(
            TestInstance::new("InventoryOpenContainerType")
                .value("isExternalContainer", TestValue::Bool(false)),
        ),
    )
}

/// Returns the `InventoryContainer` Record `name` of a closed container holding `capacity`.
/// The database needs [`entity_schema`](crate::entity_schema).
#[must_use]
pub fn closed_container_record(guid: u128, name: &str, capacity: TestCapacity) -> TestRecord {
    let unit = match capacity {
        TestCapacity::Standard(value) => TestInstance::new("SStandardCargoUnit")
            .value("standardCargoUnits", TestValue::Single(value)),
        TestCapacity::Centi(value) => {
            TestInstance::new("SCentiCargoUnit").value("centiSCU", TestValue::Int32(value))
        }
        TestCapacity::Micro(value) => {
            TestInstance::new("SMicroCargoUnit").value("microSCU", TestValue::Int32(value))
        }
    };
    container_record(
        guid,
        name,
        [0.0; 3],
        TestValue::Struct(
            TestInstance::new("InventoryClosedContainerType")
                .value("capacity", TestValue::Struct(unit)),
        ),
    )
}
