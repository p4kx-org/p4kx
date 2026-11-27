// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::resource_fixture;
use crate::resource_quality_schema_fixture::OverrideEntryTarget;

pub(crate) const ELEMENT_ALPHA: u128 = 0x1166_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const ELEMENT_DANGLING_RESOURCE: u128 = 0x1166_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const COMPOSITION: u128 = 0x1166_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const MISSING_RESOURCE: u128 = 0x1166_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const ELEMENT_UNEXPECTED_RESOURCE: u128 = 0x1166_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const INSTABILITY_BITS: u32 = 0x7fc0_1166;
pub(crate) const WIDE_QUALITY_SCALE: i64 = 9_007_199_254_740_993;
pub(crate) const DEPOSIT_KEY: &str = "@sdk_mineable_deposit";
pub(crate) const TUNINGS: [(&str, f32); 7] = [
    ("elementInstability", f32::from_bits(INSTABILITY_BITS)),
    ("elementResistance", -0.0),
    ("elementOptimalWindowMidpoint", 0.5),
    ("elementOptimalWindowMidpointRandomness", 0.125),
    ("elementOptimalWindowThinness", 0.25),
    ("elementExplosionMultiplier", 1.5),
    ("elementClusterFactor", 2.0),
];

const PART: &str = "SdkMineableCompositionPart";
const ELEMENT_PATH: &str = "libs/foundry/records/mining/mineableelements/sdk/";

/// Adds canonical mining declarations and records to the existing full resource source.
pub(crate) fn database() -> DataForgeBuilder {
    declarations(resource_fixture::database(OverrideEntryTarget::Concrete))
        .record(element(
            ELEMENT_ALPHA,
            "Alpha",
            resource_fixture::RESOURCE_ALPHA,
        ))
        .record(element(
            ELEMENT_DANGLING_RESOURCE,
            "Dangling",
            MISSING_RESOURCE,
        ))
        .record(composition())
}

/// The second eligible physical declaration with the first element's GUID.
pub(crate) fn duplicate_element() -> TestRecord {
    element(ELEMENT_ALPHA, "Alias", resource_fixture::RESOURCE_BETA)
}

/// An element whose authored `ResourceType` GUID points to an `EntityClassDefinition`.
pub(crate) fn unexpected_resource_element() -> TestRecord {
    element(
        ELEMENT_UNEXPECTED_RESOURCE,
        "Unexpected",
        resource_fixture::CONTAINER,
    )
}

fn declarations(builder: DataForgeBuilder) -> DataForgeBuilder {
    let mut element = TestStruct::new("MineableElement").field(
        "resourceType",
        TestType::ReferenceTo("ResourceType".to_owned()),
    );
    for (name, _) in TUNINGS {
        element = element.field(name, TestType::Single);
    }
    builder
        .structure(element)
        .structure(
            TestStruct::new(PART)
                .field(
                    "mineableElement",
                    TestType::ReferenceTo("MineableElement".to_owned()),
                )
                .field("minPercentage", TestType::UInt16)
                .field("maxPercentage", TestType::Int32)
                .field("probability", TestType::Single)
                .field("curveExponent", TestType::Double)
                .field("qualityScale", TestType::Int64),
        )
        .structure(
            TestStruct::new("MineableComposition")
                .field("depositName", TestType::Locale)
                .field("minimumDistinctElements", TestType::UInt16)
                .array("compositionArray", TestType::Class(PART.to_owned())),
        )
}

fn element(guid: u128, name: &str, resource: u128) -> TestRecord {
    let mut record = TestRecord::new(
        guid,
        "MineableElement",
        format!("MineableElement.Sdk{name}"),
        format!("{ELEMENT_PATH}{}.xml", name.to_ascii_lowercase()),
    )
    .value("resourceType", TestValue::Reference(resource));
    for (name, value) in TUNINGS {
        record = record.value(name, TestValue::Single(value));
    }
    record
}

fn composition() -> TestRecord {
    TestRecord::new(
        COMPOSITION,
        "MineableComposition",
        "MineableComposition.SdkComposition",
        "libs/foundry/records/mining/rockcompositionpresets/sdk/composition.xml",
    )
    .value("depositName", TestValue::text(DEPOSIT_KEY))
    .value("minimumDistinctElements", TestValue::UInt16(2))
    .value(
        "compositionArray",
        TestValue::Array(vec![
            part(ELEMENT_ALPHA, 10, 40, 0.5),
            part(ELEMENT_DANGLING_RESOURCE, 20, 70, 0.25),
            part(ELEMENT_ALPHA, 10, 40, 0.5),
        ]),
    )
}

fn part(element: u128, minimum: u16, maximum: i32, probability: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PART)
            .value("mineableElement", TestValue::Reference(element))
            .value("minPercentage", TestValue::UInt16(minimum))
            .value("maxPercentage", TestValue::Int32(maximum))
            .value("probability", TestValue::Single(probability))
            .value("curveExponent", TestValue::Double(-0.0))
            .value("qualityScale", TestValue::Int64(WIDE_QUALITY_SCALE)),
    )
}
