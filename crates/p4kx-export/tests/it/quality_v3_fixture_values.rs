// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestValue};

use crate::quality_v3_fixture_schema as schema;

pub(crate) const DISTRIBUTION: u128 = 0x51;
pub(crate) const QUANTIZATION: u128 = 0x52;
pub(crate) const RESOURCE_A: u128 = 0x71;
pub(crate) const RESOURCE_B: u128 = 0x61;

pub(crate) fn normal(minimum: TestValue, mean: TestValue, stddev: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingQualityDistributionNormal")
            .value("min", minimum)
            .value("max", TestValue::Int32(i32::MAX))
            .value("mean", mean)
            .value("stddev", stddev),
    )
}

pub(crate) fn distribution(guid: u128, name: &str, value: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        "CraftingQualityDistributionRecord",
        format!("CraftingQualityDistributionRecord.{name}"),
        format!("libs/foundry/records/crafting/qualitydistribution/{name}.xml"),
    )
    .value("qualityDistribution", value)
}

pub(crate) fn quantization(guid: u128, name: &str, path: &str, value: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        "CraftingQualityQuantizationRecord",
        format!("CraftingQualityQuantizationRecord.{name}"),
        format!("libs/foundry/records/crafting/qualityquantization/{path}.xml"),
    )
    .value("qualityQuantization", value)
}

pub(crate) fn bands(values: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingQualityQuantization").value("bands", TestValue::Array(values)),
    )
}

pub(crate) fn band(start: i32, end: i32, mapped: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingQualityQuantizationBand")
            .value("start", TestValue::Int32(start))
            .value("end", TestValue::Int32(end))
            .value("mappedValue", TestValue::Int32(mapped)),
    )
}

pub(crate) fn reference(wrapper: &str, field: &str, target: u128) -> TestValue {
    TestValue::Struct(TestInstance::new(wrapper).value(field, TestValue::Reference(target)))
}

pub(crate) fn override_record(guid: u128, name: &str, value: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        "CraftingQualityLocationOverrideRecord",
        format!("CraftingQualityLocationOverrideRecord.{name}"),
        format!("libs/foundry/records/crafting/qualitydistribution/{name}.xml"),
    )
    .value("locationOverride", value)
}

pub(crate) fn overrides(values: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingQualityLocationOverride")
            .value("locationOverrideList", TestValue::Array(values)),
    )
}

pub(crate) fn override_entry(minimum: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingQualityLocationOverrideEntry")
            .value("location", TestValue::Null)
            .value(
                "qualityDistribution",
                TestValue::Struct(
                    TestInstance::new("CraftingQualityDistributionUniform")
                        .value("min", TestValue::Int32(minimum))
                        .value("max", TestValue::Int32(100)),
                ),
            ),
    )
}

pub(crate) fn property(target: Option<u128>) -> TestValue {
    let reference = target.map_or(TestValue::Null, TestValue::Reference);
    TestValue::Struct(
        TestInstance::new("ResourceTypeCraftingData")
            .value("name", TestValue::text("quality"))
            .value(
                "qualityQuantization",
                TestValue::Struct(
                    TestInstance::new("CraftingQualityQuantization_RecordRef")
                        .value("qualityQuantizationRecord", reference),
                ),
            ),
    )
}

pub(crate) fn distribution_property(curve: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ResourceTypeCraftingData")
            .value("name", TestValue::text("curve"))
            .value("qualityDistribution", curve),
    )
}
pub(crate) fn resource(guid: u128, name: &str, properties: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{name}.xml"),
    )
    .value("properties", TestValue::Array(properties))
}

pub(crate) fn healthy() -> DataForgeBuilder {
    schema::schema()
        .record(distribution(
            DISTRIBUTION,
            "normal",
            normal(
                TestValue::Int32(i32::MIN),
                TestValue::Double(7.5),
                TestValue::Double(1.25),
            ),
        ))
        .record(quantization(
            QUANTIZATION,
            "Quantizer",
            "quantizer",
            bands(vec![band(0, 399, 264), band(400, 999, 553)]),
        ))
}

pub(crate) fn duplicate_quantizations() -> DataForgeBuilder {
    schema::schema()
        .record(quantization(
            QUANTIZATION,
            "Empty",
            "shared",
            bands(Vec::new()),
        ))
        .record(quantization(
            QUANTIZATION,
            "Winner",
            "shared",
            bands(vec![band(0, 99, 20)]),
        ))
        .record(quantization(
            QUANTIZATION,
            "Later",
            "shared",
            bands(vec![band(0, 99, 30)]),
        ))
        .record(resource(
            RESOURCE_A,
            "resource_a",
            vec![
                property(None),
                property(Some(QUANTIZATION)),
                property(Some(QUANTIZATION)),
            ],
        ))
        .record(resource(
            RESOURCE_B,
            "resource_b",
            vec![property(Some(QUANTIZATION))],
        ))
}

pub(crate) fn absent_distribution() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("CraftingQualityDistributionRecord"))
        .record(TestRecord::new(
            DISTRIBUTION,
            "CraftingQualityDistributionRecord",
            "CraftingQualityDistributionRecord.absent",
            "libs/foundry/records/crafting/qualitydistribution/absent.xml",
        ))
}

pub(crate) fn reference_forms() -> DataForgeBuilder {
    schema::schema()
        .record(distribution(
            DISTRIBUTION,
            "cycle_a",
            reference(
                "CraftingQualityDistribution_RecordRef",
                "qualityDistributionRecord",
                0x53,
            ),
        ))
        .record(distribution(
            0x53,
            "cycle_b",
            reference(
                "CraftingQualityDistribution_RecordRef",
                "qualityDistributionRecord",
                DISTRIBUTION,
            ),
        ))
        .record(override_record(
            0x54,
            "override_ref",
            reference(
                "CraftingQualityLocationOverride_RecordRef",
                "locationOverrideRecord",
                0x55,
            ),
        ))
        .record(override_record(
            0x55,
            "override_base",
            TestValue::Struct(TestInstance::new("CraftingQualityLocationOverride_Base")),
        ))
        .record(quantization(
            QUANTIZATION,
            "QuantizerRef",
            "quantizer_ref",
            reference(
                "CraftingQualityQuantization_RecordRef",
                "qualityQuantizationRecord",
                0x56,
            ),
        ))
        .record(quantization(
            0x56,
            "QuantizerBase",
            "quantizer_base",
            TestValue::Struct(TestInstance::new("CraftingQualityQuantization_Base")),
        ))
}

pub(crate) fn inverse_dependencies() -> DataForgeBuilder {
    schema::schema()
        .record(quantization(
            QUANTIZATION,
            "Quantizer",
            "quantizer",
            bands(vec![band(0, 99, 20)]),
        ))
        .record(resource(
            RESOURCE_A,
            "outside_a",
            vec![property(None), property(Some(QUANTIZATION))],
        ))
        .record(resource(
            RESOURCE_B,
            "outside_b",
            vec![property(Some(QUANTIZATION)), property(Some(QUANTIZATION))],
        ))
        .record(resource(0x81, "outside_negative", vec![property(None)]))
}
