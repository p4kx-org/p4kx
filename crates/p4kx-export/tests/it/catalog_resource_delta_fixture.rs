// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestStruct, TestType, TestValue};

use super::{MISSING, NAN_BITS, RESOURCE, class, node, pointer};

pub(super) const BASE: &str = "ItemResourceDeltaBase";
const AMOUNT: &str = "ItemResourceDeltaAmount";
const UNIT: &str = "SBaseResourceUnit";
const COMPOSITION: &str = "ItemResourceComposition";
const COMPOSITION_VALUE: &str = "ItemResourceCompositionValue";
const RESOURCE_ENUM: &str = "ResourceNetworkResource";
const GENERATION: &str = "ItemResourceDeltaGeneration";
const CONSUMPTION: &str = "ItemResourceDeltaConsumption";
const CONVERSION: &str = "ItemResourceDeltaConversion";
const STORAGE: &str = "ItemResourceDeltaStorage";
const REFLECTION: &str = "ItemResourceDeltaNetworkReflection";
const FUTURE: &str = "FixtureFutureDelta";

pub(super) fn schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    let fields = [
        TestStruct::new(BASE),
        side_fields(TestStruct::new(GENERATION).parent(BASE), "generation")
            .field("composition", class(COMPOSITION))
            .field("noOverGeneration", TestType::Bool),
        consumption_fields(TestStruct::new(CONSUMPTION).parent(BASE)),
        side_fields(
            consumption_fields(TestStruct::new(CONVERSION).parent(BASE)),
            "generation",
        )
        .field("generatedComposition", class(COMPOSITION))
        .field("noOverGeneration", TestType::Bool),
        side_fields(
            side_fields(TestStruct::new(STORAGE).parent(BASE), "generation"),
            "consumption",
        )
        .field("discharge", TestType::Bool),
        TestStruct::new(REFLECTION)
            .parent(BASE)
            .field("resource", TestType::Enum(RESOURCE_ENUM.to_owned()))
            .field("binaryEvaluation", TestType::Bool),
        TestStruct::new(FUTURE)
            .parent(BASE)
            .field("fixtureRule", TestType::Single),
        TestStruct::new(AMOUNT)
            .field("resource", TestType::Enum(RESOURCE_ENUM.to_owned()))
            .field("resourceAmountPerSecond", pointer(UNIT)),
        TestStruct::new(COMPOSITION).array("values", class(COMPOSITION_VALUE)),
        TestStruct::new(COMPOSITION_VALUE)
            .field(
                "containerResource",
                TestType::ReferenceTo("ResourceType".to_owned()),
            )
            .field("ratio", TestType::Single),
        TestStruct::new("ResourceType"),
        TestStruct::new(UNIT),
        TestStruct::new("SStandardResourceUnit")
            .parent(UNIT)
            .field("standardResourceUnits", TestType::Single),
        TestStruct::new("SPowerSegmentResourceUnit")
            .parent(UNIT)
            .field("units", TestType::Int32),
        TestStruct::new("SCentiResourceUnit")
            .parent(UNIT)
            .field("centiResourceUnits", TestType::Int32),
        TestStruct::new("SMicroResourceUnit")
            .parent(UNIT)
            .field("microResourceUnits", TestType::Int32),
        TestStruct::new("FixtureUnknownResourceUnit")
            .parent(UNIT)
            .field("fixtureAmount", TestType::Single),
    ];
    fields.into_iter().fold(
        builder.enumeration(RESOURCE_ENUM, ["Power", "Coolant", "FixtureNewResource"]),
        DataForgeBuilder::structure,
    )
}

fn side_fields(structure: TestStruct, field: &str) -> TestStruct {
    structure.field(field, class(AMOUNT))
}

fn consumption_fields(structure: TestStruct) -> TestStruct {
    side_fields(structure, "consumption")
        .field("minimumConsumptionFraction", TestType::Single)
        .field("consumptionComposition", class(COMPOSITION))
}

pub(super) fn values() -> Vec<TestValue> {
    vec![
        node(generation()),
        TestValue::Null,
        node(conversion(4, 1.25, true, MISSING)),
        node(consumption()),
        node(conversion(8, 3.5, false, RESOURCE)),
        node(storage()),
        node(
            TestInstance::new(REFLECTION)
                .value("resource", TestValue::text("FixtureNewResource"))
                .value("binaryEvaluation", TestValue::Bool(false)),
        ),
        node(
            TestInstance::new(FUTURE)
                .value("fixtureRule", TestValue::Single(f32::from_bits(NAN_BITS))),
        ),
    ]
}

fn generation() -> TestInstance {
    TestInstance::new(GENERATION)
        .value(
            "generation",
            amount(
                "Power",
                "SPowerSegmentResourceUnit",
                "units",
                TestValue::Int32(17),
            ),
        )
        .value("noOverGeneration", TestValue::Bool(false))
}

fn consumption() -> TestInstance {
    TestInstance::new(CONSUMPTION)
        .value(
            "consumption",
            amount(
                "FixtureNewResource",
                "SCentiResourceUnit",
                "centiResourceUnits",
                TestValue::Int32(0),
            ),
        )
        .value("minimumConsumptionFraction", TestValue::Single(0.0))
}

fn storage() -> TestInstance {
    TestInstance::new(STORAGE)
        .value(
            "generation",
            amount(
                "Coolant",
                "SMicroResourceUnit",
                "microResourceUnits",
                TestValue::Int32(-8),
            ),
        )
        .value(
            "consumption",
            amount(
                "Power",
                "FixtureUnknownResourceUnit",
                "fixtureAmount",
                TestValue::Single(1.0),
            ),
        )
        .value("discharge", TestValue::Bool(true))
}

fn conversion(consumed: i32, generated: f32, bounded: bool, guid: u128) -> TestInstance {
    let entry = TestInstance::new(COMPOSITION_VALUE)
        .value("containerResource", TestValue::Reference(guid))
        .value("ratio", TestValue::Single(0.5));
    let composition =
        TestInstance::new(COMPOSITION).value("values", TestValue::Array(vec![node(entry)]));
    TestInstance::new(CONVERSION)
        .value(
            "consumption",
            amount(
                "Power",
                "SPowerSegmentResourceUnit",
                "units",
                TestValue::Int32(consumed),
            ),
        )
        .value(
            "generation",
            amount(
                "Coolant",
                "SStandardResourceUnit",
                "standardResourceUnits",
                TestValue::Single(generated),
            ),
        )
        .value("generatedComposition", node(composition))
        .value("minimumConsumptionFraction", TestValue::Single(0.25))
        .value("noOverGeneration", TestValue::Bool(bounded))
}

fn amount(resource: &str, unit: &str, field: &str, value: TestValue) -> TestValue {
    node(
        TestInstance::new(AMOUNT)
            .value("resource", TestValue::text(resource))
            .value(
                "resourceAmountPerSecond",
                node(TestInstance::new(unit).value(field, value)),
            ),
    )
}
