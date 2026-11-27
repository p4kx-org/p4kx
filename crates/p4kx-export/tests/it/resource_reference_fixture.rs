// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const RESOURCE: u128 = 0xb101;
pub(crate) const DEFINITION: u128 = 0xb201;

const BASE: &str = "CraftingQualityDistribution_Base";
const NON_REF: &str = "CraftingQualityDistribution_Base_NonRef";

#[derive(Clone, Copy)]
pub(crate) enum TargetShape {
    Kept,
    Leftover,
    WrongFamily,
    InexactBounds,
}

fn schema(minimum: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ResourceType").array(
            "properties",
            TestType::StrongPointer("ResourceTypeProperties".to_owned()),
        ))
        .structure(TestStruct::new("ResourceTypeProperties").field("name", TestType::String))
        .structure(
            TestStruct::new("ResourceTypeCraftingData")
                .parent("ResourceTypeProperties")
                .field(
                    "qualityDistribution",
                    TestType::StrongPointer(BASE.to_owned()),
                ),
        )
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(NON_REF).parent(BASE))
        .structure(
            TestStruct::new("CraftingQualityDistributionNormal")
                .parent(NON_REF)
                .field("min", minimum)
                .field("max", TestType::Int32)
                .field("mean", TestType::Single)
                .field("stddev", TestType::Single),
        )
        .structure(
            TestStruct::new("CraftingQualityDistribution_RecordRef")
                .parent(BASE)
                .field("qualityDistributionRecord", TestType::Reference),
        )
        .structure(TestStruct::new("CraftingQualityDistributionRecord").field(
            "qualityDistribution",
            TestType::StrongPointer(NON_REF.to_owned()),
        ))
}

fn resource(property: TestInstance) -> TestRecord {
    TestRecord::new(
        RESOURCE,
        "ResourceType",
        "ResourceType.FixtureResource",
        "libs/foundry/records/resourcetypedatabase/included/fixture_resource.xml",
    )
    .value(
        "properties",
        TestValue::Array(vec![TestValue::Struct(property)]),
    )
}

fn linked_resource(target: u128) -> TestRecord {
    resource(
        TestInstance::new("ResourceTypeCraftingData").value(
            "qualityDistribution",
            TestValue::Struct(
                TestInstance::new("CraftingQualityDistribution_RecordRef")
                    .value("qualityDistributionRecord", TestValue::Reference(target)),
            ),
        ),
    )
}

pub(crate) fn target(shape: TargetShape) -> DataForgeBuilder {
    let (minimum_type, minimum, name, target) = match shape {
        TargetShape::Kept => (
            TestType::Int32,
            TestValue::Int32(1),
            "FixtureQuality",
            DEFINITION,
        ),
        TargetShape::Leftover => (
            TestType::Int32,
            TestValue::Int32(1),
            "Fixture_TEMPLATE",
            DEFINITION,
        ),
        TargetShape::WrongFamily => (
            TestType::Int32,
            TestValue::Int32(1),
            "FixtureQuality",
            RESOURCE,
        ),
        TargetShape::InexactBounds => (
            TestType::Single,
            TestValue::Single(1.0),
            "FixtureQuality",
            DEFINITION,
        ),
    };
    schema(minimum_type).record(linked_resource(target)).record(
        TestRecord::new(
            DEFINITION,
            "CraftingQualityDistributionRecord",
            format!("CraftingQualityDistributionRecord.{name}"),
            format!("libs/foundry/records/crafting/qualitydistribution/excluded/{name}.xml"),
        )
        .value(
            "qualityDistribution",
            TestValue::Struct(
                TestInstance::new("CraftingQualityDistributionNormal")
                    .value("min", minimum)
                    .value("max", TestValue::Int32(10))
                    .value("mean", TestValue::Single(5.0))
                    .value("stddev", TestValue::Single(1.0)),
            ),
        ),
    )
}

pub(crate) fn unknown_property() -> DataForgeBuilder {
    schema(TestType::Int32)
        .structure(
            TestStruct::new("FixtureFutureProperty")
                .parent("ResourceTypeProperties")
                .field("opaque", TestType::Int32),
        )
        .record(resource(
            TestInstance::new("FixtureFutureProperty")
                .value("name", TestValue::text("future"))
                .value("opaque", TestValue::Int32(42)),
        ))
}
