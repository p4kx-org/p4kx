// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::resource_fixture_values as values;
use crate::resource_quality_schema_fixture::OverrideEntryTarget;
use crate::resource_schema_fixture;

pub(crate) const RESOURCE_ALPHA: u128 = 0x7001;
pub(crate) const RESOURCE_BETA: u128 = 0x7002;
pub(crate) const RESOURCE_UNUSED: u128 = 0x7003;
pub(crate) const COMMODITY_ALPHA: u128 = 0x7101;
pub(crate) const COMMODITY_BETA: u128 = 0x7102;
pub(crate) const COMMODITY_NULL: u128 = 0x7103;
pub(crate) const CONTAINER: u128 = 0x7201;
pub(crate) const LOCATION: u128 = 0x7301;
pub(crate) const GROUP_ALPHA: u128 = 0x7401;
pub(crate) const GROUP_BETA: u128 = 0x7402;
pub(crate) const DISTRIBUTION: u128 = 0x7501;
pub(crate) const OVERRIDE: u128 = 0x7502;
pub(crate) const QUANTIZATION: u128 = 0x7503;

const RESOURCES: &str = "libs/foundry/records/resourcetypedatabase/";
const CRAFTING: &str = "libs/foundry/records/crafting/";

/// Produces a complete synthetic source for both override-pointer declarations.
pub(crate) fn database(target: OverrideEntryTarget) -> DataForgeBuilder {
    let builder = resource_schema_fixture::declarations(DataForgeBuilder::new().version(8), target);
    let builder = external_types(builder);
    let builder = resources(builder);
    let builder = groups(builder);
    definitions(builder)
        .record(commodity(
            COMMODITY_ALPHA,
            "FixtureAlpha",
            Some(RESOURCE_ALPHA),
        ))
        .record(commodity(
            COMMODITY_BETA,
            "FixtureBeta",
            Some(RESOURCE_ALPHA),
        ))
        .record(commodity(COMMODITY_NULL, "FixtureUnlinked", None))
        .record(TestRecord::new(
            CONTAINER,
            "EntityClassDefinition",
            "EntityClassDefinition.FixtureContainer",
            "libs/foundry/records/entities/fixture/container.xml",
        ))
        .record(TestRecord::new(
            LOCATION,
            "StarMapObject",
            "StarMapObject.FixtureLocation",
            "libs/foundry/records/starmap/pu/fixture_location.xml",
        ))
}

fn external_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("FixtureComponent"))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("FixtureComponent".to_owned()),
        ))
        .structure(
            TestStruct::new("ResourceContainer")
                .parent("FixtureComponent")
                .array(
                    "defaultComposition",
                    TestType::Class("FixtureCompositionEntry".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("FixtureCompositionEntry")
                .field("entry", TestType::ReferenceTo("ResourceType".to_owned())),
        )
        .structure(TestStruct::new("StarMapObject"))
        .structure(TestStruct::new("CraftingQualityDistributionRecord").field(
            "qualityDistribution",
            TestType::StrongPointer("CraftingQualityDistribution_Base_NonRef".to_owned()),
        ))
        .structure(
            TestStruct::new("CraftingQualityLocationOverrideRecord").field(
                "locationOverride",
                TestType::StrongPointer("CraftingQualityLocationOverride_Base_NonRef".to_owned()),
            ),
        )
        .structure(TestStruct::new("CraftingQualityQuantizationRecord").field(
            "qualityQuantization",
            TestType::StrongPointer("CraftingQualityQuantization_Base_NonRef".to_owned()),
        ))
}

fn resources(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(resource_alpha())
        .record(resource_beta())
        .record(
            resource(RESOURCE_UNUSED, "FixtureResourceUnused")
                .value(
                    "densityType",
                    values::owned(
                        "ResourceTypeMolarMass",
                        [("molarMass", TestValue::Single(6.75))],
                    ),
                )
                .value("properties", TestValue::Array(Vec::new())),
        )
}

fn resource_alpha() -> TestRecord {
    resource(RESOURCE_ALPHA, "FixtureResourceAlpha")
        .value(
            "densityType",
            density("GramsPerCubicCentimeter", "gramsPerCubicCentimeter", 3.25),
        )
        .value("validateDefaultCargoBox", TestValue::Bool(false))
        .value("refinedVersion", TestValue::Reference(RESOURCE_BETA))
        .value(
            "defaultCargoContainers",
            values::owned(
                "SResourceTypeDefaultCargoContainers",
                [
                    ("one_eighthSCU", TestValue::Reference(CONTAINER)),
                    ("twoSCU", TestValue::Reference(CONTAINER)),
                ],
            ),
        )
        .value(
            "properties",
            TestValue::Array(vec![
                volatility("", 0.0, 0.25),
                crafting_alpha(),
                TestValue::Null,
                volatility("Repeated", 7.0, 0.0),
            ]),
        )
}

fn resource_beta() -> TestRecord {
    resource(RESOURCE_BETA, "FixtureResourceBeta")
        .value(
            "densityType",
            density("KilogramsPerCubicMeter", "kilogramsPerCubicMeter", 12.5),
        )
        .value(
            "properties",
            TestValue::Array(vec![
                crafting_beta(),
                values::owned(
                    "ResourceTypeCraftingData",
                    [
                        ("name", TestValue::text("Quantized")),
                        ("qualityQuantization", quantization_reference()),
                    ],
                ),
            ]),
        )
}

fn density(unit: &str, field: &'static str, value: f32) -> TestValue {
    values::owned(
        "ResourceTypeDensity",
        [(
            "densityUnit",
            values::owned(unit, [(field, TestValue::Single(value))]),
        )],
    )
}

fn volatility(name: &str, value: f32, decay: f32) -> TestValue {
    values::owned(
        "ResourceTypeVolatility",
        [
            ("name", TestValue::text(name)),
            ("volatility", TestValue::Single(value)),
            ("healthDecayPerSecond", TestValue::Single(decay)),
        ],
    )
}
fn resource(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("{RESOURCES}{}.xml", name.to_ascii_lowercase()),
    )
    .value("displayName", TestValue::text("@fixture_resource_name"))
    .value(
        "description",
        TestValue::text("@fixture_resource_description"),
    )
    .value(
        "defaultThumbnailPath",
        TestValue::text("textures/fixture/resource.dds"),
    )
    .value("defaultThumbnailPathSVG", TestValue::text(""))
    .value("rttThumbnailEntityClass", TestValue::Reference(CONTAINER))
}

fn crafting_alpha() -> TestValue {
    values::owned(
        "ResourceTypeCraftingData",
        [
            ("name", TestValue::text("Authored")),
            (
                "qualityDistribution",
                values::normal(16_777_217, i32::MAX, 9.5, 2.25),
            ),
            (
                "qualityLocationOverride",
                values::owned(
                    "CraftingQualityLocationOverride",
                    [(
                        "locationOverrideList",
                        TestValue::Array(vec![
                            values::override_entry(LOCATION, distribution_reference()),
                            TestValue::Null,
                        ]),
                    )],
                ),
            ),
            ("qualityQuantization", quantization_reference()),
        ],
    )
}

fn crafting_beta() -> TestValue {
    values::owned(
        "ResourceTypeCraftingData",
        [
            ("name", TestValue::text("Authored")),
            ("qualityDistribution", distribution_reference()),
            (
                "qualityLocationOverride",
                values::quality_reference(
                    "CraftingQualityLocationOverride_RecordRef",
                    "locationOverrideRecord",
                    OVERRIDE,
                ),
            ),
            ("qualityQuantization", values::quantization()),
        ],
    )
}

fn distribution_reference() -> TestValue {
    values::quality_reference(
        "CraftingQualityDistribution_RecordRef",
        "qualityDistributionRecord",
        DISTRIBUTION,
    )
}

fn quantization_reference() -> TestValue {
    values::quality_reference(
        "CraftingQualityQuantization_RecordRef",
        "qualityQuantizationRecord",
        QUANTIZATION,
    )
}

fn groups(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(
            group(GROUP_ALPHA, "FixtureGroupAlpha")
                .value(
                    "resources",
                    TestValue::Array(vec![
                        TestValue::Reference(RESOURCE_ALPHA),
                        TestValue::Null,
                        TestValue::Reference(RESOURCE_ALPHA),
                        TestValue::Reference(RESOURCE_BETA),
                    ]),
                )
                .value(
                    "groups",
                    TestValue::Array(vec![TestValue::Reference(GROUP_BETA)]),
                ),
        )
        .record(
            group(GROUP_BETA, "FixtureGroupBeta")
                .value(
                    "resources",
                    TestValue::Array(vec![TestValue::Reference(RESOURCE_ALPHA)]),
                )
                .value(
                    "groups",
                    TestValue::Array(vec![TestValue::Reference(GROUP_ALPHA)]),
                ),
        )
}

fn group(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceTypeGroup",
        format!("ResourceTypeGroup.{name}"),
        format!("{RESOURCES}{}.xml", name.to_ascii_lowercase()),
    )
    .value("displayName", TestValue::text("@fixture_group_name"))
    .value("description", TestValue::text("@fixture_group_description"))
    .value(
        "defaultThumbnailPath",
        TestValue::text("textures/fixture/group.dds"),
    )
    .value("defaultThumbnailPathSVG", TestValue::text(""))
}

fn definitions(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(
            TestRecord::new(
                DISTRIBUTION,
                "CraftingQualityDistributionRecord",
                "CraftingQualityDistributionRecord.FixtureUniform",
                format!("{CRAFTING}qualitydistribution/fixture_uniform.xml"),
            )
            .value("qualityDistribution", values::uniform(i32::MIN, 16_777_217)),
        )
        .record(
            TestRecord::new(
                OVERRIDE,
                "CraftingQualityLocationOverrideRecord",
                "CraftingQualityLocationOverrideRecord.FixtureOverride",
                format!("{CRAFTING}qualitydistribution/fixture_override.xml"),
            )
            .value(
                "locationOverride",
                values::owned(
                    "CraftingQualityLocationOverride",
                    [(
                        "locationOverrideList",
                        TestValue::Array(vec![values::override_entry(
                            LOCATION,
                            values::normal(0, 12, 4.5, 1.0),
                        )]),
                    )],
                ),
            ),
        )
        .record(
            TestRecord::new(
                QUANTIZATION,
                "CraftingQualityQuantizationRecord",
                "CraftingQualityQuantizationRecord.FixtureQuantization",
                format!("{CRAFTING}qualityquantization/fixture_quantization.xml"),
            )
            .value("qualityQuantization", values::quantization()),
        )
}

fn commodity(guid: u128, name: &str, resource: Option<u128>) -> TestRecord {
    let composition = resource
        .into_iter()
        .map(|guid| {
            values::owned(
                "FixtureCompositionEntry",
                [("entry", TestValue::Reference(guid))],
            )
        })
        .collect();
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{name}"),
        format!(
            "libs/foundry/records/entities/commodities/{}.xml",
            name.to_ascii_lowercase()
        ),
    )
    .value(
        "Components",
        TestValue::Array(vec![values::owned(
            "ResourceContainer",
            [("defaultComposition", TestValue::Array(composition))],
        )]),
    )
}
