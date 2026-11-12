// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The structs of the reputation fixture.

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn reputation_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("FactionReputation")
            .field("displayName", TestType::Locale)
            .field("isNPC", TestType::Bool)
            .field("reputationContextPropertiesUI", TestType::Reference)
            .array("allies", TestType::Reference)
            .array("enemies", TestType::Reference)
            .field("hostilityParams", class("RelationStandingParams"))
            .field("alliedParams", class("RelationStandingParams"))
            .array(
                "propertiesBB",
                pointer("SReputationContextBBPropertyParams"),
            ),
        TestStruct::new("RelationStandingParams")
            .field("scope", TestType::Reference)
            .field("standing", TestType::Reference),
        TestStruct::new("SReputationContextBBPropertyParams")
            .field("name", TestType::String)
            .field("dynamicProperty", pointer("SBBDynamicProperty")),
        TestStruct::new("SBBDynamicProperty"),
        TestStruct::new("SBBDynamicPropertyBool")
            .parent("SBBDynamicProperty")
            .field("value", TestType::Bool),
        TestStruct::new("SBBDynamicPropertyLocString")
            .parent("SBBDynamicProperty")
            .field("value", TestType::Locale),
        TestStruct::new("SReputationScopeParams")
            .field("scopeName", TestType::String)
            .field("displayName", TestType::Locale)
            .field("standingMap", class("SReputationStandingMapParams")),
        TestStruct::new("SReputationStandingMapParams")
            .field("reputationCeiling", TestType::Int32)
            .field("initialReputation", TestType::Int32)
            .array("standings", TestType::Reference),
        TestStruct::new("SReputationStandingParams")
            .field("name", TestType::String)
            .field("description", TestType::String)
            .field("perkDescription", TestType::Locale)
            .field("icon", TestType::String)
            .field("missionBoardIcon", TestType::String)
            .field("displayName", TestType::Locale)
            .field("minReputation", TestType::Int64)
            .field("driftReputation", TestType::Int64)
            .field("driftTimeHours", TestType::Single)
            .field("gated", TestType::Bool),
        TestStruct::new("SReputationContextUI")
            .field("primaryScopeContext", class("SReputationScopeContextUI"))
            .array("scopeContextList", class("SReputationScopeContextUI")),
        TestStruct::new("SReputationScopeContextUI").field("scope", TestType::Reference),
    ]
}

fn mission_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("Placeholder"),
        TestStruct::new("Faction").field(
            "factionReputationRef",
            TestType::ReferenceTo("FactionReputation".to_owned()),
        ),
        TestStruct::new("MissionType").field("LocalisedTypeName", TestType::Locale),
        TestStruct::new("MissionLocality").array("availableLocations", TestType::Reference),
        TestStruct::new("ScenarioProgress")
            .array("factionRewardTiers", class("SScenarioProgressRewardsTiers")),
        TestStruct::new("SScenarioProgressRewardsTiers")
            .field("convertTiersPointsToPercent", TestType::Bool)
            .field("faction", TestType::Reference)
            .array("tierProgressions", class("STierProgressions")),
        TestStruct::new("STierProgressions")
            .field("minMultiplierThreshold", TestType::Single)
            .field("completionType", pointer("CompletionType"))
            .field("periodMissionScenario", TestType::Reference)
            .array("tierRewards", class("STierReward")),
        TestStruct::new("CompletionType"),
        TestStruct::new("CompletionType_GlobalProgression").parent("CompletionType"),
        TestStruct::new("STierReward")
            .field("minPoints", TestType::Int32)
            .field("badgeToAward", TestType::String)
            .field("globalPointsMultiplier", TestType::Single)
            .array("blueprintPool", TestType::Reference),
    ]
}

/// Adds the structs of the fixture to `builder`.
pub(super) fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [reputation_structs(), mission_structs()]
        .into_iter()
        .flatten()
        .fold(builder, DataForgeBuilder::structure)
}
