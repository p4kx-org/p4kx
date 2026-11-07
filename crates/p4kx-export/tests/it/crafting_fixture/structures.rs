// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn cost(name: &str) -> TestStruct {
    TestStruct::new(name).parent("CraftingCost")
}

/// Adds the structs of the fixture to `builder`.
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    crafting_structures(commodity_structures(builder))
}

fn commodity_structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        TestStruct::new("EntityClassDefinition")
            .array("StaticEntityClassData", pointer("StaticEntityClassData"))
            .array("Components", pointer("DataForgeComponentParams")),
        TestStruct::new("StaticEntityClassData"),
        TestStruct::new("EntityUIDisplayParams")
            .parent("StaticEntityClassData")
            .field("displayDescription", TestType::Locale),
        TestStruct::new("DataForgeComponentParams"),
        component("SCItemPurchasableParams")
            .field("displayName", TestType::Locale)
            .field("displayType", TestType::Locale),
        component("ResourceContainer").array(
            "defaultComposition",
            class("ResourceContainerDefaultCompositionEntry"),
        ),
        TestStruct::new("ResourceContainerDefaultCompositionEntry")
            .field("entry", TestType::Reference),
        TestStruct::new("ResourceType")
            .field("displayName", TestType::Locale)
            .field("description", TestType::Locale)
            .field("densityType", pointer("ResourceTypeDensityType"))
            .field("refinedVersion", TestType::Reference),
        TestStruct::new("ResourceTypeDensityType"),
        TestStruct::new("ResourceTypeDensity")
            .parent("ResourceTypeDensityType")
            .field("densityUnit", pointer("BaseDensityUnit")),
        TestStruct::new("BaseDensityUnit"),
        TestStruct::new("GramsPerCubicCentimeter")
            .parent("BaseDensityUnit")
            .field("gramsPerCubicCentimeter", TestType::Single),
        TestStruct::new("ResourceTypeGroup")
            .field("displayName", TestType::Locale)
            .array("groups", TestType::Reference)
            .array("resources", TestType::Reference),
        TestStruct::new("BlueprintCategoryRecord"),
        TestStruct::new("BlueprintCategoryDatabaseRecord").array("categories", TestType::Reference),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::structure)
}

fn crafting_structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        TestStruct::new("CraftingBlueprintRecord")
            .field("blueprint", pointer("CraftingBlueprintBase")),
        TestStruct::new("CraftingBlueprintBase"),
        TestStruct::new("CraftingBlueprint")
            .parent("CraftingBlueprintBase")
            .field("blueprintName", TestType::Locale)
            .field("category", TestType::Reference)
            .field("processSpecificData", pointer("CraftingProcess"))
            .array("tiers", class("CraftingBlueprintTier")),
        TestStruct::new("GenericCraftingBlueprint")
            .parent("CraftingBlueprintBase")
            .field("processSpecificData", pointer("CraftingProcess")),
        TestStruct::new("CraftingProcess"),
        TestStruct::new("CraftingProcess_Creation")
            .parent("CraftingProcess")
            .field("entityClass", TestType::Reference),
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
        TestStruct::new("CraftingCost"),
        cost("CraftingCost_Select")
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
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::structure)
}
