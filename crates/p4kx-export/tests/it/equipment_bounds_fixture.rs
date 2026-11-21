// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1166_b001;
const TARGET: u128 = 0x1166_b002;
const CARGO: &str = "SBaseCargoUnit";

pub(super) fn complete() -> DataForgeBuilder {
    let mut components: Vec<_> = capacities()
        .into_iter()
        .map(|capacity| attachment(Some(capacity)))
        .collect();
    components.push(attachment(Some(
        TestInstance::new("SMicroCargoUnit").value("microSCU", TestValue::Int32(0)),
    )));
    components.push(metadata(false));
    components.push(metadata(true));
    components.push(node(
        TestInstance::new("VehicleComponentParams")
            .value("maxBoundingBoxSize", vector3([5.0, 6.0, 7.0])),
    ));
    schema()
        .record(owner("FixtureAllBounds", "all_bounds", components))
        .record(owner(
            "FixtureBoundsAlias",
            "bounds_alias",
            vec![attachment(None)],
        ))
        .record(TestRecord::new(
            TARGET,
            "EntityClassDefinition",
            "EntityClassDefinition.FixtureTarget",
            "libs/foundry/records/entities/scitem/fixture/bounds_target.xml",
        ))
}

fn schema() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::String)
                .field("inventoryOccupancyVolume", pointer(CARGO))
                .field("inventoryOccupancyDimensions", class("Vec3"))
                .field("inventoryOccupancyLocalBoundsMin", class("Vec3"))
                .field("inventoryOccupancyLocalBoundsMax", class("Vec3"))
                .field("inventoryOccupancyDimensionsUIOverride", pointer("Vec3"))
                .field("inventoryOccupancyFixedGridDimensionsUIV4", class("Vec2")),
        )
        .structure(
            TestStruct::new("VehicleComponentParams")
                .parent("DataForgeComponentParams")
                .field("maxBoundingBoxSize", class("Vec3")),
        )
        .structure(
            TestStruct::new("SEntityComponentObjectMetadataParams")
                .parent("DataForgeComponentParams")
                .field("hasLocalBounds", TestType::Bool),
        );
    units(vectors(builder))
}

fn vectors(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("Vec2")
                .field("x", TestType::Single)
                .field("y", TestType::Single)
                .field(
                    "futureTarget",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("Vec3")
                .field("x", TestType::Single)
                .field("y", TestType::Single)
                .field("z", TestType::Single)
                .field("futureMarker", TestType::Bool),
        )
}

fn units(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(CARGO))
        .structure(
            TestStruct::new("SCentiCargoUnit")
                .parent(CARGO)
                .field("centiSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new("SMicroCargoUnit")
                .parent(CARGO)
                .field("microSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new("SStandardCargoUnit")
                .parent(CARGO)
                .field("standardCargoUnits", TestType::Single),
        )
        .structure(
            TestStruct::new("FixtureFutureCargoUnit")
                .parent(CARGO)
                .field("futureAmount", TestType::UInt64)
                .field(
                    "futureTarget",
                    TestType::ReferenceTo("EntityClassDefinition".to_owned()),
                ),
        )
}

fn capacities() -> [TestInstance; 5] {
    [
        TestInstance::new(CARGO),
        TestInstance::new("SCentiCargoUnit").value("centiSCU", TestValue::Int32(-125)),
        TestInstance::new("SMicroCargoUnit").value("microSCU", TestValue::Int32(0)),
        TestInstance::new("SStandardCargoUnit")
            .value("standardCargoUnits", TestValue::Single(-0.0)),
        TestInstance::new("FixtureFutureCargoUnit")
            .value("futureAmount", TestValue::UInt64(u64::MAX))
            .value("futureTarget", TestValue::Reference(TARGET)),
    ]
}

fn attachment(capacity: Option<TestInstance>) -> TestValue {
    let volume = capacity.map_or(TestValue::Null, node);
    let fixed = TestInstance::new("Vec2")
        .value("x", TestValue::Single(4.0))
        .value("y", TestValue::Single(-0.0))
        .value("futureTarget", TestValue::Reference(TARGET));
    let definition = TestInstance::new("SItemDefinition")
        .value("Type", TestValue::text("Clothing"))
        .value("inventoryOccupancyVolume", volume)
        .value(
            "inventoryOccupancyDimensions",
            vector3([0.0, -0.0, f32::from_bits(0x7fc0_1166)]),
        )
        .value(
            "inventoryOccupancyLocalBoundsMin",
            vector3([-1.0, -2.0, -3.0]),
        )
        .value("inventoryOccupancyLocalBoundsMax", vector3([1.0, 2.0, 3.0]))
        .value("inventoryOccupancyDimensionsUIOverride", TestValue::Null)
        .value("inventoryOccupancyFixedGridDimensionsUIV4", node(fixed));
    node(TestInstance::new("SAttachableComponentParams").value("AttachDef", node(definition)))
}

fn owner(name: &str, path: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        OWNER,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{path}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn vector3(values: [f32; 3]) -> TestValue {
    node(
        TestInstance::new("Vec3")
            .value("x", TestValue::Single(values[0]))
            .value("y", TestValue::Single(values[1]))
            .value("z", TestValue::Single(values[2]))
            .value("futureMarker", TestValue::Bool(false)),
    )
}

fn metadata(value: bool) -> TestValue {
    node(
        TestInstance::new("SEntityComponentObjectMetadataParams")
            .value("hasLocalBounds", TestValue::Bool(value)),
    )
}
fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}
fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
