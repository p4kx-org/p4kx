// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) fn database(broken_distribution: bool) -> DataForgeBuilder {
    let distribution = TestStruct::new("CraftingQualityDistributionRecord").field(
        "qualityDistribution",
        pointer("CraftingQualityDistribution_Base"),
    );
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(if broken_distribution {
            distribution.declared_size(1)
        } else {
            distribution
        })
        .structure(TestStruct::new("CraftingQualityDistribution_Base"))
        .structure(TestStruct::new("CraftingQualityLocationOverride_Base"))
        .structure(
            TestStruct::new("CraftingQualityLocationOverrideRecord").field(
                "locationOverride",
                pointer("CraftingQualityLocationOverride_Base"),
            ),
        )
        .structure(TestStruct::new("CraftingQualityQuantizationRecord").field(
            "qualityQuantization",
            pointer("CraftingQualityQuantization_Base"),
        ))
        .structure(TestStruct::new("CraftingQualityQuantization_Base"))
        .structure(
            TestStruct::new("CraftingQualityQuantizationBand")
                .field("start", TestType::Int32)
                .field("end", TestType::Int32)
                .field("mappedValue", TestType::Int32),
        )
        .structure(
            TestStruct::new("CraftingQualityQuantization")
                .parent("CraftingQualityQuantization_Base")
                .array(
                    "bands",
                    TestType::Class("CraftingQualityQuantizationBand".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("ResourceType")
                .declared_size(1)
                .field("value", TestType::UInt64),
        )
        .structure(
            TestStruct::new("Foreign")
                .declared_size(1)
                .field("value", TestType::UInt64),
        );
    records(builder)
}

fn records(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(distribution(1, "Primary"))
        .record(distribution(2, "Sibling"))
        .record(
            record(
                3,
                "CraftingQualityQuantizationRecord",
                "crafting/qualityquantization/primary.xml",
            )
            .value(
                "qualityQuantization",
                TestValue::Struct(TestInstance::new("CraftingQualityQuantization").value(
                    "bands",
                    TestValue::Array(vec![TestValue::Struct(
                            TestInstance::new("CraftingQualityQuantizationBand")
                                .value("start", TestValue::Int32(0))
                                .value("end", TestValue::Int32(1000))
                                .value("mappedValue", TestValue::Int32(500)),
                        )]),
                )),
            ),
        )
        .record(record(
            4,
            "CraftingQualityQuantizationRecord",
            "crafting/qualityquantization/sibling.xml",
        ))
        .record(record(5, "ResourceType", "resourcetypedatabase/unread.xml"))
        .record(record(6, "Foreign", "unrelated/unread.xml"))
        .record(
            record(
                7,
                "CraftingQualityLocationOverrideRecord",
                "crafting/qualitydistribution/override.xml",
            )
            .value(
                "locationOverride",
                TestValue::Struct(TestInstance::new("CraftingQualityLocationOverride_Base")),
            ),
        )
        .record(distribution(1, "Alias"))
        .record(distribution(8, "Dev"))
}

fn distribution(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "CraftingQualityDistributionRecord",
        format!("CraftingQualityDistributionRecord.{name}"),
        format!(
            "libs/foundry/records/crafting/qualitydistribution/{}.xml",
            name.to_ascii_lowercase()
        ),
    )
    .value(
        "qualityDistribution",
        TestValue::Struct(TestInstance::new("CraftingQualityDistribution_Base")),
    )
}

fn record(guid: u128, source_type: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        source_type,
        format!("{source_type}.Primary"),
        format!("libs/foundry/records/{path}"),
    )
}

fn pointer(source_type: &str) -> TestType {
    TestType::StrongPointer(source_type.to_owned())
}
