// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn value(name: &str) -> TestStruct {
    TestStruct::new(name).parent("MissionPropertyValue")
}

/// Adds the structs of the fixture to `builder`.
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        handler_structs(),
        generator_structs(),
        prerequisite_structs(),
        blueprint_structs(),
        property_structs(),
        record_structs(),
    ]
    .into_iter()
    .flatten()
    .fold(builder, DataForgeBuilder::structure)
}

fn prerequisite_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractPrerequisite"),
        TestStruct::new("ContractPrerequisite_Locality")
            .parent("ContractPrerequisite")
            .field("localityAvailable", TestType::Reference),
        TestStruct::new("ContractPrerequisite_Location")
            .parent("ContractPrerequisite")
            .field("locationAvailable", TestType::Reference),
        TestStruct::new("ContractPrerequisite_Reputation")
            .parent("ContractPrerequisite")
            .field("exclude", TestType::Bool)
            .field("factionReputation", TestType::Reference)
            .field("scope", TestType::Reference)
            .field("minStanding", TestType::Reference)
            .field("maxStanding", TestType::Reference),
    ]
}

fn blueprint_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("MissionModifier"),
        TestStruct::new("MissionModifier_TemporaryBlueprint")
            .parent("MissionModifier")
            .field("enabled", TestType::Bool)
            .field("blueprintPool", TestType::Reference),
        TestStruct::new("BlueprintRewards")
            .parent("ContractResult")
            .field("chance", TestType::Single)
            .array("missionResults", class("Bool"))
            .field("blueprintPool", TestType::Reference),
    ]
}

fn handler_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractGenerator")
            .array("generators", pointer("ContractGeneratorHandler")),
        TestStruct::new("ContractGeneratorHandler"),
        TestStruct::new("ContractGeneratorHandler_Career")
            .parent("ContractGeneratorHandler")
            .field("debugName", TestType::String)
            .field("factionReputation", TestType::Reference)
            .field("defaultAvailability", class("ContractAvailability"))
            .field("contractParams", class("ContractParamOverrides"))
            .array("contracts", class("CareerContract"))
            .array("introContracts", class("CareerContract")),
        TestStruct::new("ContractGeneratorHandler_List")
            .parent("ContractGeneratorHandler")
            .field("debugName", TestType::String)
            .field("factionReputation", TestType::Reference)
            .field(
                "defaultAvailability",
                class("ContractAvailabilityWithoutCooldown"),
            )
            .field("contractParams", class("ContractParamOverrides"))
            .array("contracts", class("CareerContract"))
            .array("introContracts", class("CareerContract")),
        TestStruct::new("ContractAvailability")
            .field("hasPersonalCooldown", TestType::Bool)
            .field("personalCooldownTime", TestType::Single)
            .field("personalCooldownTimeVariation", TestType::Single)
            .field("abandonedCooldownTime", TestType::Single)
            .field("abandonedCooldownTimeVariation", TestType::Single)
            .array("prerequisites", pointer("ContractPrerequisite")),
        TestStruct::new("ContractGeneratorHandler_PVPBountyDef")
            .parent("ContractGeneratorHandler")
            .field("debugName", TestType::String)
            .field("factionReputation", TestType::Reference)
            .field("defaultAvailability", class("ContractAvailabilityPartial"))
            .field("contractParams", class("ContractParamOverrides"))
            .array("contracts", class("CareerContract"))
            .array("introContracts", class("CareerContract")),
        TestStruct::new("ContractAvailabilityPartial")
            .field("hasPersonalCooldown", TestType::Bool)
            .field("personalCooldownTime", TestType::Int32)
            .array("prerequisites", pointer("ContractPrerequisite")),
        TestStruct::new("ContractAvailabilityWithoutCooldown")
            .array("prerequisites", pointer("ContractPrerequisite")),
    ]
}

fn generator_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractParamOverrides")
            .field("missionTypeOverride", TestType::Reference)
            .array("modifierOverrides", pointer("MissionModifier"))
            .array("stringParamOverrides", class("ContractStringParam"))
            .array("propertyOverrides", class("MissionProperty")),
        TestStruct::new("ContractStringParam")
            .field("param", TestType::String)
            .field("value", TestType::Locale),
        TestStruct::new("CareerContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("notForRelease", TestType::Bool)
            .field("template", TestType::Reference)
            .field("missionBrokerEntry", TestType::Reference)
            .field("minStanding", TestType::Reference)
            .field("maxStanding", TestType::Reference)
            .field("paramOverrides", class("ContractParamOverrides"))
            .array("subContracts", class("SubContract"))
            .array("additionalPrerequisites", pointer("ContractPrerequisite"))
            .field("contractResults", class("ContractResultBlock")),
        TestStruct::new("SubContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .array("additionalPrerequisites", pointer("ContractPrerequisite"))
            .field("paramOverrides", class("ContractParamOverrides"))
            .field("contractResults", class("ContractResultBlock")),
        TestStruct::new("MissionLocality").array("availableLocations", TestType::Reference),
        TestStruct::new("StarMapObjectType").field("name", TestType::String),
        TestStruct::new("StarMapObject")
            .field("name", TestType::Locale)
            .field(
                "type",
                TestType::ReferenceTo("StarMapObjectType".to_owned()),
            )
            .field("parent", TestType::ReferenceTo("StarMapObject".to_owned()))
            .field("hideInStarmap", TestType::Bool),
        TestStruct::new("ContractResultBlock").array("contractResults", pointer("ContractResult")),
        TestStruct::new("ContractResult"),
        TestStruct::new("ContractResult_LegacyReputation")
            .parent("ContractResult")
            .array("missionResults", class("Bool"))
            .array("contractResultReputationAmounts", class("ReputationAmount")),
        TestStruct::new("ContractResult_Reward")
            .parent("ContractResult")
            .array("missionResults", class("Bool"))
            .field("contractReward", pointer("MissionReward")),
        TestStruct::new("MissionReward").field("reward", TestType::Int32),
        TestStruct::new("MissionRewardWithCurrency")
            .parent("MissionReward")
            .field("max", TestType::Int32)
            .field("plusBonuses", TestType::Bool)
            .field("currencyType", TestType::Enum("ECurrency".to_owned())),
        TestStruct::new("ContractResult_CompletionTags").parent("ContractResult"),
        TestStruct::new("Bool").field("value", TestType::Bool),
        TestStruct::new("ReputationAmount")
            .field("factionReputation", TestType::Reference)
            .field("reputationScope", TestType::Reference)
            .field("reward", TestType::Reference),
    ]
}

fn property_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("MissionProperty")
            .field("missionVariableName", TestType::String)
            .field("extendedTextToken", TestType::String)
            .field("value", pointer("MissionPropertyValue")),
        TestStruct::new("MissionPropertyValue"),
        value("MissionPropertyValue_StringHash")
            .array("options", class("MissionPropertyValueOption_StringHash")),
        TestStruct::new("MissionPropertyValueOption_StringHash")
            .field("textId", TestType::Locale)
            .field("weighting", TestType::Single),
        value("MissionPropertyValue_Organization")
            .array("matchConditions", pointer("DataSetMatchCondition")),
        TestStruct::new("DataSetMatchCondition"),
        TestStruct::new("DataSetMatchCondition_SpecificOrganizationsDef")
            .parent("DataSetMatchCondition")
            .array("organizations", TestType::Reference),
        value("MissionPropertyValue_Location"),
        value("MissionPropertyValue_AIName"),
        value("MissionPropertyValue_ShipSpawnDescriptions"),
        value("MissionPropertyValue_Boolean").field("value", TestType::Bool),
    ]
}

fn record_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ContractTemplate")
            .field("contractDisplayInfo", class("ContractDisplayInfo"))
            .array("contractProperties", class("MissionProperty")),
        TestStruct::new("ContractDisplayInfo")
            .field("type", TestType::Reference)
            .array("displayString", class("LocID")),
        TestStruct::new("LocID").field("value", TestType::Locale),
        TestStruct::new("MissionType").field("LocalisedTypeName", TestType::Locale),
        TestStruct::new("MissionOrganization")
            .field("stringVariants", class("MissionStringVariants")),
        TestStruct::new("MissionStringVariants").array("variants", class("MissionStringVariant")),
        TestStruct::new("MissionStringVariant")
            .field("tag", TestType::Reference)
            .field("string", TestType::Locale),
        TestStruct::new("FactionReputation").field("displayName", TestType::Locale),
        TestStruct::new("MissionBrokerEntry")
            .field("title", TestType::Locale)
            .field("description", TestType::Locale)
            .field("type", TestType::Reference)
            .field("locationMissionAvailable", TestType::Reference),
        TestStruct::new("ReputationScope"),
        TestStruct::new("ReputationReward").field("reputationAmount", TestType::Int32),
        TestStruct::new("Tag")
            .field("tagName", TestType::String)
            .field("legacyGUID", TestType::UInt32)
            .array("children", TestType::ReferenceTo("Tag".into())),
        TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".into())),
    ]
}
