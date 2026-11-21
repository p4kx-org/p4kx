// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

pub(crate) fn database() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("FixtureComponent"))
        .structure(TestStruct::new("FixtureUnrelated").parent("FixtureComponent"))
        .structure(TestStruct::new("FixtureDisplay"))
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array(
                    "Components",
                    TestType::StrongPointer("FixtureComponent".to_owned()),
                )
                .array(
                    "StaticEntityClassData",
                    TestType::StrongPointer("FixtureDisplay".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("SCItemPurchasableParams")
                .parent("FixtureComponent")
                .field("displayName", TestType::Locale)
                .field("displayType", TestType::Locale),
        )
        .structure(
            TestStruct::new("EntityUIDisplayParams")
                .parent("FixtureDisplay")
                .field("displayDescription", TestType::Locale),
        )
        .structure(
            TestStruct::new("ResourceContainer")
                .parent("FixtureComponent")
                .array(
                    "defaultComposition",
                    TestType::StrongPointer("FixtureComposition".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("FixtureComposition")
                .field("entry", TestType::ReferenceTo("ResourceType".to_owned())),
        );
    resources(builder)
}

fn resources(builder: DataForgeBuilder) -> DataForgeBuilder {
    let builder = builder
        .structure(
            TestStruct::new("ResourceType")
                .field("displayName", TestType::Locale)
                .field("description", TestType::Locale)
                .field(
                    "densityType",
                    TestType::StrongPointer("ResourceTypeDensityType".to_owned()),
                )
                .field(
                    "refinedVersion",
                    TestType::ReferenceTo("ResourceType".to_owned()),
                )
                .array(
                    "properties",
                    TestType::StrongPointer("ResourceTypeProperties".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("ResourceTypeGroup")
                .array(
                    "resources",
                    TestType::ReferenceTo("ResourceType".to_owned()),
                )
                .array(
                    "groups",
                    TestType::ReferenceTo("ResourceTypeGroup".to_owned()),
                ),
        )
        .structure(TestStruct::new("ResourceTypeDensityType"))
        .structure(
            TestStruct::new("ResourceTypeDensity")
                .parent("ResourceTypeDensityType")
                .field(
                    "densityUnit",
                    TestType::StrongPointer("BaseDensityUnit".to_owned()),
                ),
        )
        .structure(TestStruct::new("BaseDensityUnit"))
        .structure(
            TestStruct::new("GramsPerCubicCentimeter")
                .parent("BaseDensityUnit")
                .field("gramsPerCubicCentimeter", TestType::Single),
        )
        .structure(TestStruct::new("ResourceTypeProperties").field("name", TestType::String))
        .structure(
            TestStruct::new("ResourceTypeCraftingData")
                .parent("ResourceTypeProperties")
                .field(
                    "qualityQuantization",
                    TestType::StrongPointer("CraftingQualityQuantization_Base".to_owned()),
                ),
        );
    quantization(builder)
}

fn quantization(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("CraftingQualityQuantization_Base"))
        .structure(
            TestStruct::new("CraftingQualityQuantization_RecordRef")
                .parent("CraftingQualityQuantization_Base")
                .field(
                    "qualityQuantizationRecord",
                    TestType::ReferenceTo("CraftingQualityQuantizationRecord".to_owned()),
                ),
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
            TestStruct::new("CraftingQualityQuantizationBand")
                .field("start", TestType::Int32)
                .field("end", TestType::Int32)
                .field("mappedValue", TestType::Int32),
        )
        .structure(TestStruct::new("CraftingQualityQuantizationRecord").field(
            "qualityQuantization",
            TestType::StrongPointer("CraftingQualityQuantization_Base".to_owned()),
        ))
}
