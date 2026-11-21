// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

pub(crate) fn schema() -> DataForgeBuilder {
    numeric_schema(TestType::Int32, TestType::Double, TestType::Double)
}

pub(crate) fn numeric_schema(
    minimum: TestType,
    mean: TestType,
    stddev: TestType,
) -> DataForgeBuilder {
    complete_schema(distribution_types(minimum, mean, stddev).structure(distribution_record()))
}

pub(crate) fn uncaptured_curve_schema() -> DataForgeBuilder {
    let record = distribution_record().field(
        "spareCurve",
        TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
    );
    complete_schema(
        distribution_types(TestType::Int32, TestType::Double, TestType::Double).structure(record),
    )
}

fn complete_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    quantization_schema(override_schema(builder))
}

fn distribution_record() -> TestStruct {
    TestStruct::new("CraftingQualityDistributionRecord").field(
        "qualityDistribution",
        TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
    )
}

fn distribution_types(minimum: TestType, mean: TestType, stddev: TestType) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("CraftingQualityDistribution_Base"))
        .structure(
            TestStruct::new("CraftingQualityDistribution_Base_NonRef")
                .parent("CraftingQualityDistribution_Base"),
        )
        .structure(
            TestStruct::new("CraftingQualityDistributionNormal")
                .parent("CraftingQualityDistribution_Base_NonRef")
                .field("min", minimum)
                .field("max", TestType::Int32)
                .field("mean", mean)
                .field("stddev", stddev),
        )
        .structure(
            TestStruct::new("CraftingQualityDistributionUniform")
                .parent("CraftingQualityDistribution_Base_NonRef")
                .field("min", TestType::Int32)
                .field("max", TestType::Int32),
        )
        .structure(
            TestStruct::new("CraftingQualityDistribution_RecordRef")
                .parent("CraftingQualityDistribution_Base")
                .field(
                    "qualityDistributionRecord",
                    TestType::ReferenceTo("CraftingQualityDistributionRecord".to_owned()),
                ),
        )
}

fn override_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("CraftingQualityLocationOverride_Base"))
        .structure(
            TestStruct::new("CraftingQualityLocationOverride_Base_NonRef")
                .parent("CraftingQualityLocationOverride_Base"),
        )
        .structure(TestStruct::new("CraftingQualityLocationOverrideEntry_Base"))
        .structure(
            TestStruct::new("CraftingQualityLocationOverrideEntry")
                .parent("CraftingQualityLocationOverrideEntry_Base")
                .field("location", TestType::Reference)
                .field(
                    "qualityDistribution",
                    TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("CraftingQualityLocationOverride")
                .parent("CraftingQualityLocationOverride_Base_NonRef")
                .array(
                    "locationOverrideList",
                    TestType::StrongPointer("CraftingQualityLocationOverrideEntry_Base".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("CraftingQualityLocationOverride_RecordRef")
                .parent("CraftingQualityLocationOverride_Base")
                .field(
                    "locationOverrideRecord",
                    TestType::ReferenceTo("CraftingQualityLocationOverrideRecord".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("CraftingQualityLocationOverrideRecord").field(
                "locationOverride",
                TestType::StrongPointer("CraftingQualityLocationOverride_Base".to_owned()),
            ),
        )
}

fn quantization_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("CraftingQualityQuantization_Base"))
        .structure(
            TestStruct::new("CraftingQualityQuantization_Base_NonRef")
                .parent("CraftingQualityQuantization_Base"),
        )
        .structure(
            TestStruct::new("CraftingQualityQuantizationBand")
                .field("start", TestType::Int32)
                .field("end", TestType::Int32)
                .field("mappedValue", TestType::Int32),
        )
        .structure(
            TestStruct::new("CraftingQualityQuantization")
                .parent("CraftingQualityQuantization_Base_NonRef")
                .array(
                    "bands",
                    TestType::Class("CraftingQualityQuantizationBand".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("CraftingQualityQuantization_RecordRef")
                .parent("CraftingQualityQuantization_Base")
                .field(
                    "qualityQuantizationRecord",
                    TestType::ReferenceTo("CraftingQualityQuantizationRecord".to_owned()),
                ),
        )
        .structure(TestStruct::new("CraftingQualityQuantizationRecord").field(
            "qualityQuantization",
            TestType::StrongPointer("CraftingQualityQuantization_Base".to_owned()),
        ))
        .structure(
            TestStruct::new("ResourceTypeCraftingData")
                .field("name", TestType::String)
                .field(
                    "qualityDistribution",
                    TestType::StrongPointer("CraftingQualityDistribution_Base".to_owned()),
                )
                .field(
                    "qualityQuantization",
                    TestType::StrongPointer("CraftingQualityQuantization_Base".to_owned()),
                ),
        )
        .structure(TestStruct::new("ResourceType").array(
            "properties",
            TestType::Class("ResourceTypeCraftingData".to_owned()),
        ))
}
