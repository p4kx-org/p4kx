// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The structs of the loot fixture.

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn selector_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("TagsDNFTerm")
            .array("positiveTags", TestType::Reference)
            .array("negativeTags", TestType::Reference),
        TestStruct::new("LootArchetypeV3Selector"),
        TestStruct::new("LootArchetypeV3Selector_Tags")
            .parent("LootArchetypeV3Selector")
            .field("tags", class("TagsDNFTerm")),
        TestStruct::new("EntityClassList"),
        TestStruct::new("EntityClassList_Manual")
            .parent("EntityClassList")
            .array("entityClasses", TestType::Reference),
        TestStruct::new("LootArchetypeV3Selector_EntityClasses")
            .parent("LootArchetypeV3Selector")
            .field("entityClasses", pointer("EntityClassList")),
        TestStruct::new("SpawnWithV3Selector"),
        TestStruct::new("SpawnWithV3Selector_Tags")
            .parent("SpawnWithV3Selector")
            .field("tags", class("TagsDNFTerm")),
        TestStruct::new("QuantityRange"),
        TestStruct::new("QuantityRange_Linear")
            .parent("QuantityRange")
            .field("min", TestType::Single)
            .field("max", TestType::Single),
    ]
}

fn archetype_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ArchetypeOptionalDataV3"),
        TestStruct::new("ArchetypeOptionalDataV3_StackSize")
            .parent("ArchetypeOptionalDataV3")
            .field("stackSize", pointer("QuantityRange")),
        TestStruct::new("ArchetypeOptionalDataV3_SpawnWith")
            .parent("ArchetypeOptionalDataV3")
            .field("name", TestType::String)
            .field("chanceToSpawnWith", TestType::Single)
            .field("mode", TestType::String)
            .field("amountToSpawn", pointer("QuantityRange"))
            .field("selector", pointer("SpawnWithV3Selector")),
        TestStruct::new("LootArchetypeV3Entry")
            .field("name", TestType::String)
            .field("weight", TestType::Single)
            .field("selector", pointer("LootArchetypeV3Selector"))
            .array("optionalData", pointer("ArchetypeOptionalDataV3")),
        TestStruct::new("LootArchetypeV3Base"),
        TestStruct::new("LootArchetypeV3")
            .parent("LootArchetypeV3Base")
            .array("entries", class("LootArchetypeV3Entry")),
        TestStruct::new("LootArchetypeV3_RecordRef")
            .parent("LootArchetypeV3Base")
            .field("lootArchetypeRecord", TestType::Reference),
        TestStruct::new("LootArchetypeV3Record").field("lootArchetype", class("LootArchetypeV3")),
    ]
}

fn table_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("LootTableOptionalDataV3"),
        TestStruct::new("LootTableOptionalDataV3_FixtureExtension")
            .parent("LootTableOptionalDataV3")
            .field("unknownField", TestType::UInt16),
        TestStruct::new("LootTableOptionalDataV3_PriorityChoice")
            .parent("LootTableOptionalDataV3")
            .field("priorityCount", TestType::Int32),
        TestStruct::new("LootTableOptionalDataV3_ChoiceLimit")
            .parent("LootTableOptionalDataV3")
            .field("choiceLimit", TestType::Int32),
        TestStruct::new("LootTableOptionalDataV3_DupeLimit")
            .parent("LootTableOptionalDataV3")
            .field("dupeLimit", TestType::Int32),
        TestStruct::new("LootTableOptionalDataV3_ChanceToExist")
            .parent("LootTableOptionalDataV3")
            .field("chanceToExist", TestType::Single),
        TestStruct::new("LootTableV3Entry")
            .field("name", TestType::String)
            .field("weight", TestType::Single)
            .field("archetype", pointer("LootArchetypeV3Base"))
            .array("optionalData", pointer("LootTableOptionalDataV3")),
        TestStruct::new("LootTableV3").array("lootArchetypes", class("LootTableV3Entry")),
        TestStruct::new("LootTableV3Record").field("lootTable", class("LootTableV3")),
    ]
}

fn legacy_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("NumResultsConstraints")
            .field("minResults", TestType::Int32)
            .field("maxResults", TestType::Int32),
        TestStruct::new("WeightedLootArchetype")
            .field("weight", TestType::Single)
            .field("archetype", TestType::Reference)
            .field("numberOfResultsConstraints", class("NumResultsConstraints")),
        TestStruct::new("LootTable").array("lootArchetypes", class("WeightedLootArchetype")),
        TestStruct::new("TagList").array("tags", TestType::Reference),
        TestStruct::new("EntryOptionalData"),
        TestStruct::new("EntryOptionalData_StackSize")
            .parent("EntryOptionalData")
            .field("min", TestType::Int32)
            .field("max", TestType::Int32),
        TestStruct::new("EntryOptionalData_SpawnWith")
            .parent("EntryOptionalData")
            .field("name", TestType::String)
            .field("mode", TestType::String)
            .field("min", TestType::Int32)
            .field("max", TestType::Int32)
            .field("tagsToMatch", class("TagsDNFTerm")),
        TestStruct::new("LootArchetypeEntry_Primary")
            .field("name", TestType::String)
            .field("weight", TestType::Single)
            .field("tag", TestType::Reference)
            .field("additionalTags", class("TagsDNFTerm"))
            .array("optionalData", pointer("EntryOptionalData")),
        TestStruct::new("LootArchetypeOrGroup_Primary")
            .array("entries", class("LootArchetypeEntry_Primary")),
        TestStruct::new("LootArchetypeEntry_Secondary")
            .field("weight", TestType::Single)
            .field("tag", TestType::Reference),
        TestStruct::new("LootArchetypeOrGroup_Secondary")
            .field("groupName", TestType::String)
            .array("entries", class("LootArchetypeEntry_Secondary")),
        TestStruct::new("LootArchetype")
            .field("excludedTags", class("TagList"))
            .field("primaryOrGroup", class("LootArchetypeOrGroup_Primary"))
            .array("secondaryOrGroups", class("LootArchetypeOrGroup_Secondary")),
    ]
}

fn filter_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("PoolFilter"),
        TestStruct::new("PoolFilterInstance")
            .field("name", TestType::String)
            .field("mode", TestType::String)
            .field("filter", pointer("PoolFilter")),
        TestStruct::new("PoolFilter_Sequence")
            .parent("PoolFilter")
            .array("filters", class("PoolFilterInstance")),
        TestStruct::new("PoolFilter_RecordRef")
            .parent("PoolFilter")
            .field("filterRecord", TestType::Reference),
        TestStruct::new("PoolFilter_Tags")
            .parent("PoolFilter")
            .field("tagFilter", class("TagsDNFTerm")),
        TestStruct::new("PoolFilterRecord").field("filter", pointer("PoolFilter")),
    ]
}

fn slot_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("FloatFactorRange")
            .field("min", TestType::Single)
            .field("max", TestType::Single),
        TestStruct::new("LootConstraints")
            .field("totalResultsLimit", TestType::Int32)
            .field("chanceToGenerate", TestType::Single)
            .field(
                "chanceToGenerateAdditionalAttachedInventories",
                TestType::Single,
            )
            .field("poolFilter", TestType::Reference)
            .field("fullnessFactorRange", class("FloatFactorRange")),
        TestStruct::new("LootConfig")
            .field("lootConstraints", class("LootConstraints"))
            .field("lootTable", TestType::Reference)
            .field("lootTableV3", TestType::Reference),
        TestStruct::new("SubHarvestableEntry")
            .field("harvestable", TestType::Reference)
            .field("lootConfig", class("LootConfig")),
        TestStruct::new("SubHarvestableManual")
            .array("subHarvestables", class("SubHarvestableEntry")),
        TestStruct::new("SubHarvestableConfigSingleManual")
            .field("subConfigManual", class("SubHarvestableManual")),
        TestStruct::new("HarvestableTagList").array("tags", TestType::Reference),
        TestStruct::new("TaggedSubConfig")
            .field("name", TestType::String)
            .field("subConfig", pointer("SubHarvestableConfigSingleManual"))
            .field("tagList", pointer("HarvestableTagList")),
        TestStruct::new("SubHarvestableMultiConfig")
            .array("taggedConfigs", class("TaggedSubConfig")),
        TestStruct::new("SubHarvestableMultiConfigRecord")
            .field("multiConfig", class("SubHarvestableMultiConfig")),
        TestStruct::new("Tag"),
        TestStruct::new("StarMapObject")
            .field("name", TestType::Locale)
            .field("description", TestType::Locale),
    ]
}

/// Adds the structs of the fixture to `builder`.
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        selector_structs(),
        archetype_structs(),
        table_structs(),
        legacy_structs(),
        filter_structs(),
        slot_structs(),
    ]
    .into_iter()
    .flatten()
    .fold(builder, DataForgeBuilder::structure)
}
