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

fn faction_structs() -> [TestStruct; 6] {
    [
        TestStruct::new("FactionReputation")
            .field("displayName", TestType::Locale)
            .field("isNPC", TestType::Bool)
            .field(
                "reputationContextPropertiesUI",
                TestType::ReferenceTo("SReputationContextUI".to_owned()),
            )
            .array(
                "allies",
                TestType::ReferenceTo("FactionReputation".to_owned()),
            )
            .array(
                "enemies",
                TestType::ReferenceTo("FactionReputation".to_owned()),
            )
            .field("hostilityParams", class("RelationStandingParams"))
            .field("alliedParams", class("RelationStandingParams"))
            .array("propertiesBB", class("SReputationContextBBPropertyParams")),
        TestStruct::new("RelationStandingParams")
            .field(
                "scope",
                TestType::ReferenceTo("SReputationScopeParams".to_owned()),
            )
            .field(
                "standing",
                TestType::ReferenceTo("SReputationStandingParams".to_owned()),
            ),
        TestStruct::new("SReputationContextBBPropertyParams")
            .field("name", TestType::String)
            .field("dynamicProperty", pointer("SBBDynamicPropertyBase")),
        TestStruct::new("SBBDynamicPropertyBase"),
        TestStruct::new("SBBDynamicPropertyBool")
            .parent("SBBDynamicPropertyBase")
            .field("value", TestType::Bool),
        TestStruct::new("SBBDynamicPropertyLocString")
            .parent("SBBDynamicPropertyBase")
            .field("value", TestType::Locale),
    ]
}

fn reputation_structs() -> Vec<TestStruct> {
    faction_structs()
        .into_iter()
        .chain([
            TestStruct::new("SReputationScopeParams")
                .field("scopeName", TestType::String)
                .field("displayName", TestType::Locale)
                .field("description", TestType::Locale)
                .field("icon", TestType::String)
                .field("standingMap", class("SReputationStandingMapParams")),
            TestStruct::new("SReputationStandingMapParams")
                .field("reputationCeiling", TestType::Int64)
                .field("initialReputation", TestType::Int64)
                .array(
                    "standings",
                    TestType::ReferenceTo("SReputationStandingParams".to_owned()),
                ),
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
        ])
        .collect()
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
