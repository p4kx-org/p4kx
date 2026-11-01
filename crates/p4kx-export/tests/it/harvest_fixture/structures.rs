// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The structs of the harvest fixture.

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn element_struct() -> TestStruct {
    TestStruct::new("HarvestableElement")
        .field("relativeProbability", TestType::Single)
        .field("harvestable", TestType::Reference)
        .field("harvestableEntityClass", TestType::Reference)
        .field("clustering", TestType::Reference)
        .array("geometries", class("HarvestableGeometry"))
}

#[expect(clippy::too_many_lines, reason = "one entry per struct of the fixture")]
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        TestStruct::new("ResourceType"),
        TestStruct::new("MineableElement").field("resourceType", TestType::Reference),
        TestStruct::new("MineableCompositionPart")
            .field("minPercentage", TestType::Single)
            .field("maxPercentage", TestType::Single)
            .field("probability", TestType::Single)
            .field("mineableElement", TestType::Reference),
        TestStruct::new("MineableComposition")
            .field("depositName", TestType::Locale)
            .field("minimumDistinctElements", TestType::Int32)
            .array("compositionArray", class("MineableCompositionPart")),
        TestStruct::new("HitConsistencyParams").field("timeExponent", TestType::Single),
        TestStruct::new("MiningGlobalParams")
            .field("cSCUPerVolume", TestType::Single)
            .field("compositionMassMultiplier", TestType::Single)
            .field("defaultMass", TestType::Single)
            .field("hitConsistencyParams", class("HitConsistencyParams"))
            .field("wasteResourceType", TestType::Reference),
        TestStruct::new("EntityClassDefinition")
            .array("Components", pointer("DataForgeComponentParams")),
        TestStruct::new("DataForgeComponentParams"),
        TestStruct::new("MineableParams")
            .parent("DataForgeComponentParams")
            .field("composition", TestType::Reference)
            .field("globalParams", TestType::Reference),
        TestStruct::new("SSCSignatureSystemBaseSignatureParams")
            .array("signatures", TestType::Single),
        TestStruct::new("SSCRadarContactProperites")
            .field("contactType", TestType::Reference)
            .field(
                "baseSignatureParams",
                class("SSCSignatureSystemBaseSignatureParams"),
            ),
        TestStruct::new("SSCSignatureSystemParams")
            .parent("DataForgeComponentParams")
            .field("radarProperties", class("SSCRadarContactProperites")),
        TestStruct::new("RadarContactTypeEntry"),
        TestStruct::new("SignatureTypeGlobalParams").field("displayName", TestType::Locale),
        TestStruct::new("SignatureSystemGlobalParams")
            .array("signatureTypeParams", class("SignatureTypeGlobalParams")),
        TestStruct::new("RadarSystemGlobalParams").field(
            "signatureSystemParams",
            class("SignatureSystemGlobalParams"),
        ),
        TestStruct::new("HarvestablePreset").field("entityClass", TestType::Reference),
        TestStruct::new("HarvestableGeometry").field("tag", TestType::Reference),
        element_struct(),
        TestStruct::new("HarvestableElementGroup")
            .field("groupName", TestType::String)
            .field("groupProbability", TestType::Single)
            .array("harvestables", class("HarvestableElement")),
        TestStruct::new("HarvestableAreaTypeObjectPreset")
            .array("objectPresetPaths", TestType::String),
        TestStruct::new("HarvestableAreaPreset")
            .field("debugGroupName", TestType::String)
            .field("globalModifier", TestType::Single)
            .field("areaType", pointer("HarvestableAreaTypeObjectPreset")),
        TestStruct::new("HarvestableProviderPreset")
            .array("harvestableGroups", class("HarvestableElementGroup"))
            .array("areas", class("HarvestableAreaPreset")),
        TestStruct::new("SubHarvestableConfigSingleRef").field("subConfigRef", TestType::Reference),
        TestStruct::new("SubHarvestableManual")
            .field("configRespawnTimeMultiplier", TestType::Single)
            .field("initialSlotsProbability", TestType::Single)
            .array("subHarvestables", class("SubHarvestableEntry")),
        TestStruct::new("SubHarvestableEntry")
            .field("harvestable", TestType::Reference)
            .field("harvestableRespawnTimeMultiplier", TestType::Single)
            .field("relativeProbability", TestType::Single)
            .field("relativeProbabilityDeepest", pointer("OptionalProbability")),
        TestStruct::new("OptionalProbability").field("probability", TestType::Single),
        TestStruct::new("SubHarvestableConfigSingleManual")
            .field("subConfigManual", class("SubHarvestableManual")),
        TestStruct::new("HarvestableTagList").array("tags", TestType::Reference),
        TestStruct::new("TaggedSubConfig")
            .field("name", TestType::String)
            .field("subConfig", pointer("SubHarvestableConfigSingleManual"))
            .field("tagList", pointer("HarvestableTagList")),
        TestStruct::new("SubHarvestableMultiConfig")
            .field("defaultConfig", pointer("SubHarvestableConfigSingleRef"))
            .field("ignoreAttachableTagsForTaggedConfigs", TestType::Bool)
            .array("taggedConfigs", class("TaggedSubConfig")),
        TestStruct::new("SubHarvestableMultiConfigRecord")
            .field("multiConfig", class("SubHarvestableMultiConfig")),
        TestStruct::new("StarMapObject")
            .field("name", TestType::Locale)
            .field("description", TestType::Locale),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::structure)
}
