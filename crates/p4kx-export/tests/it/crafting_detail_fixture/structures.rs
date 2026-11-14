// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn cost(name: &str) -> TestStruct {
    TestStruct::new(name).parent("CraftingCost")
}

fn context(name: &str) -> TestStruct {
    TestStruct::new(name).parent("CraftingCostContext")
}

fn range(name: &str) -> TestStruct {
    TestStruct::new(name)
        .parent("CraftingGameplayPropertyModifierValueRange")
        .field("startQuality", TestType::Int32)
        .field("endQuality", TestType::Int32)
}

/// Adds the structs of the fixture to `builder`.
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        blueprint_structs(),
        cost_structs(),
        reward_structs(),
        quality_structs(),
    ]
    .into_iter()
    .flatten()
    .fold(builder, DataForgeBuilder::structure)
}

fn blueprint_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("Stub"),
        TestStruct::new("CraftingBlueprintRecord")
            .field("blueprint", pointer("CraftingBlueprintBase")),
        TestStruct::new("CraftingBlueprintBase"),
        TestStruct::new("CraftingBlueprint")
            .parent("CraftingBlueprintBase")
            .field("processSpecificData", pointer("CraftingProcess"))
            .array("tiers", class("CraftingBlueprintTier")),
        TestStruct::new("GenericCraftingBlueprint")
            .parent("CraftingBlueprintBase")
            .field("processSpecificData", pointer("CraftingProcess")),
        TestStruct::new("CraftingProcess"),
        TestStruct::new("CraftingProcess_Creation")
            .parent("CraftingProcess")
            .field("entityClass", TestType::Reference),
        TestStruct::new("GenericCraftingProcess_Dismantle")
            .parent("CraftingProcess")
            .field("efficiency", TestType::Single)
            .field("dismantleTime", class("TimeValue_Partitioned")),
        TestStruct::new("CraftingBlueprintTier").field("recipe", class("CraftingRecipe")),
        TestStruct::new("CraftingRecipe").field("costs", class("CraftingRecipeCosts")),
        TestStruct::new("CraftingRecipeCosts")
            .field("craftTime", class("TimeValue_Partitioned"))
            .field("mandatoryCost", pointer("CraftingCost"))
            .array("optionalCosts", pointer("CraftingCost")),
        TestStruct::new("TimeValue_Partitioned")
            .field("days", TestType::Int32)
            .field("hours", TestType::Int32)
            .field("minutes", TestType::Int32)
            .field("seconds", TestType::Single),
    ]
}

fn cost_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("CraftingCost").array("context", pointer("CraftingCostContext")),
        cost("CraftingCost_Select")
            .field("count", TestType::Int32)
            .field("nameInfo", class("CraftingNameInfo"))
            .array("options", pointer("CraftingCost")),
        TestStruct::new("CraftingNameInfo")
            .field("debugName", TestType::String)
            .field("displayName", TestType::Locale),
        cost("CraftingCost_Resource")
            .field("minQuality", TestType::Int32)
            .field("resource", TestType::Reference)
            .field("quantity", class("SStandardCargoUnit")),
        TestStruct::new("SStandardCargoUnit").field("standardCargoUnits", TestType::Single),
        cost("CraftingCost_Item")
            .field("quantity", TestType::Int32)
            .field("minQuality", TestType::Int32)
            .field("entityClass", TestType::Reference),
        TestStruct::new("CraftingCostContext"),
        context("CraftingCostContext_QuantityMultiplier").field("multiplier", TestType::Single),
        context("CraftingCostContext_ResultCompositionInclusion").field("option", TestType::String),
        context("CraftingCostContext_ResultGameplayPropertyModifiers").field(
            "gameplayPropertyModifiers",
            pointer("CraftingGameplayPropertyModifiers"),
        ),
        TestStruct::new("CraftingGameplayPropertyModifiers"),
        TestStruct::new("CraftingGameplayPropertyModifiers_List")
            .parent("CraftingGameplayPropertyModifiers")
            .array(
                "gameplayPropertyModifiers",
                pointer("CraftingGameplayPropertyModifierCommon"),
            ),
        TestStruct::new("CraftingGameplayPropertyModifierCommon")
            .field("gameplayPropertyRecord", TestType::Reference)
            .array(
                "valueRanges",
                pointer("CraftingGameplayPropertyModifierValueRange"),
            ),
        TestStruct::new("CraftingGameplayPropertyModifierValueRange"),
        range("CraftingGameplayPropertyModifierValueRange_Linear")
            .field("modifierAtStart", TestType::Single)
            .field("modifierAtEnd", TestType::Single),
        range("CraftingGameplayPropertyModifierValueRange_LinearIntegerAdditive")
            .field("additiveModifierAtStart", TestType::Int32)
            .field("additiveModifierAtEnd", TestType::Int32),
        range("CraftingGameplayPropertyModifierValueRange_Curve"),
        range("CraftingGameplayPropertyModifierValueRange_Stepped"),
    ]
}

fn reward_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("BlueprintPoolRecord")
            .array("blueprintRewards", class("BlueprintReward"))
            .field("selfLink", TestType::Reference),
        TestStruct::new("BlueprintReward")
            .field("weight", TestType::Single)
            .field("blueprintRecord", TestType::Reference),
        TestStruct::new("ContractGenerator")
            .array("generators", pointer("ContractGeneratorHandler")),
        TestStruct::new("ContractGeneratorHandler"),
        TestStruct::new("ContractGeneratorHandler_Legacy")
            .parent("ContractGeneratorHandler")
            .field("debugName", TestType::String)
            .array("legacyContracts", class("ContractLegacy")),
        TestStruct::new("ContractLegacy")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .array("results", pointer("BlueprintRewards")),
        TestStruct::new("BlueprintRewards")
            .field("chance", TestType::Single)
            .field("blueprintPool", TestType::Reference),
    ]
}

fn quality_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("StarMapObject"),
        TestStruct::new("CraftingQualityDistributionRecord").field(
            "qualityDistribution",
            pointer("CraftingQualityDistribution"),
        ),
        TestStruct::new("CraftingQualityDistribution"),
        TestStruct::new("CraftingQualityDistributionNormal")
            .parent("CraftingQualityDistribution")
            .field("min", TestType::Int32)
            .field("max", TestType::Int32)
            .field("mean", TestType::Single)
            .field("stddev", TestType::Double),
        TestStruct::new("CraftingQualityLocationOverrideRecord")
            .field("locationOverride", class("CraftingQualityLocationOverride")),
        TestStruct::new("CraftingQualityLocationOverride").array(
            "locationOverrideList",
            class("CraftingQualityLocationOverrideEntry"),
        ),
        TestStruct::new("CraftingQualityLocationOverrideEntry")
            .field("location", TestType::Reference)
            .field(
                "qualityDistribution",
                pointer("CraftingQualityDistribution"),
            ),
        TestStruct::new("CraftingGameplayPropertyDef").field("propertyName", TestType::Locale),
        TestStruct::new("CraftingGlobalParams")
            .array("dismantleBlacklistResources", TestType::Reference)
            .array("dismantleBlacklistEntityClasses", TestType::Reference)
            .field(
                "defaultBlueprintSelection",
                pointer("DefaultBlueprintSelection"),
            ),
        TestStruct::new("DefaultBlueprintSelection"),
        TestStruct::new("DefaultBlueprintSelection_Whitelist")
            .parent("DefaultBlueprintSelection")
            .array("blueprintRecords", TestType::Reference),
    ]
}
