// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The structs of the mission broker entry Records of the fixture.

use p4kx_testkit::{TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

/// Returns the structs of the fixture's mission broker entries.
pub(super) fn structs() -> Vec<TestStruct> {
    [entry(), terms(), requirements(), modifiers()]
        .into_iter()
        .flatten()
        .collect()
}

/// The Bool fields of the entry.
const FLAGS: [&str; 26] = [
    "notForRelease",
    "missionModulePerPlayer",
    "moduleHandlesOwnShutdown",
    "preShowObjectives",
    "showLifeTimeInMobiGlas",
    "canBeShared",
    "hasCompleteButton",
    "onlyOwnerCanComplete",
    "initiallyActive",
    "showAsOffer",
    "requestOnly",
    "onceOnly",
    "notifyOnAvailable",
    "availableInPrison",
    "lawfulMission",
    "tutorial",
    "canReacceptAfterAbandoning",
    "canReacceptAfterFailing",
    "applyCompletionTagsOnAbandoned",
    "applyCompletionTagsOnFailed",
    "hasPersonalCooldown",
    "instanceHasLifeTime",
    "refundBuyInOnWithdraw",
    "failIfSentToPrison",
    "failIfBecameCriminal",
    "failIfLeavePrison",
];

fn entry() -> Vec<TestStruct> {
    let entry = TestStruct::new("MissionBrokerEntry")
        .field("playerFacingDebugName", TestType::String)
        .field("title", TestType::Locale)
        .field("titleHUD", TestType::Locale)
        .field("description", TestType::Locale)
        .field("missionGiver", TestType::Locale)
        .field("commsChannelName", TestType::Locale)
        .field("missionGiverFragmentTags", TestType::String)
        .field("missionModule", TestType::String)
        .field("owner", TestType::Reference)
        .field("missionDifficulty", TestType::Int32)
        .field("missionBuyInAmount", TestType::Int32)
        .field("personalCooldownTime", TestType::Single)
        .field("personalCooldownTimeVariation", TestType::Single)
        .field("abandonedCooldownTime", TestType::Single)
        .field("abandonedCooldownTimeVariation", TestType::Single)
        .field("respawnTime", TestType::Single)
        .field("respawnTimeVariation", TestType::Single)
        .field("maxInstances", TestType::Int32)
        .field("maxInstancesPerPlayer", TestType::Int32)
        .field("maxPlayersPerInstance", TestType::Int32)
        .field("instanceLifeTime", TestType::Single)
        .field("instanceLifeTimeVariation", TestType::Single);
    let entry = FLAGS
        .into_iter()
        .fold(entry, |entry, flag| entry.field(flag, TestType::Bool));
    vec![nested(links(entry))]
}

fn links(entry: TestStruct) -> TestStruct {
    entry
        .field("type", TestType::Reference)
        .field("locationMissionAvailable", TestType::Reference)
        .field("localityAvailable", TestType::Reference)
        .field("missionGiverRecord", TestType::Reference)
        .field("invitationMission", TestType::Reference)
        .field("linkedMission", TestType::Reference)
        .array("associatedMissions", TestType::Reference)
        .array("requiredMissions", TestType::Reference)
        .array(
            "onlyAvailableIfAllMissionsNotAvailable",
            TestType::Reference,
        )
        .field("completionTags", class("TagList"))
        .array("missionTags", TestType::Reference)
        .field("requiredAreaTags", class("TagList"))
        .field("excludedAreaTags", class("TagList"))
        .array("requiredCompletedMissionTags", class("TagSearchTerm"))
        .array("journalEntriesToAddOnComplete", TestType::Reference)
        .array("journalEntriesToRemoveOnComplete", TestType::Reference)
}

fn nested(entry: TestStruct) -> TestStruct {
    entry
        .field("reputationPrerequisites", class("ReputationPrerequisites"))
        .array("availableDateSchedule", class("DateSchedule"))
        .field("missionReward", class("BrokerReward"))
        .field("missionDeadline", class("BrokerDeadline"))
        .field("reputationRequirements", pointer("ReputationRequirements"))
        .array(
            "missionResultReputationRewards",
            class("ReputationResultSet"),
        )
        .array("properties", class("MissionProperty"))
        .array("objectiveTokens", class("ObjectiveToken"))
        .field("partialRewardPayout", pointer("PartialContractRewards"))
        .array("modifiers", pointer("MissionModifier"))
        .field("missionFlow", class("MissionFlow"))
}

fn terms() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ReputationPrerequisites").field("wantedLevel", class("WantedLevel")),
        TestStruct::new("WantedLevel")
            .field("minValue", TestType::Single)
            .field("maxValue", TestType::Single),
        TestStruct::new("DateSchedule")
            .field("start", class("DateTimeStamp"))
            .field("end", class("DateTimeStamp"))
            .field("repeatEnd", class("DateStamp"))
            .field("repeatType", TestType::String),
        TestStruct::new("DateTimeStamp")
            .field("date", class("DateStamp"))
            .field("time", class("TimeStamp")),
        TestStruct::new("DateStamp")
            .field("year", TestType::Int32)
            .field("month", TestType::String)
            .field("day", TestType::Int32),
        TestStruct::new("TimeStamp")
            .field("hour", TestType::Int32)
            .field("minute", TestType::Int32)
            .field("second", TestType::Int32),
        TestStruct::new("BrokerReward")
            .field("currencyType", TestType::String)
            .field("reward", TestType::Int32)
            .field("max", TestType::Int32)
            .field("plusBonuses", TestType::Bool)
            .field("reputationBonus", TestType::Reference),
        TestStruct::new("BrokerDeadline")
            .field("missionAutoEnd", TestType::Bool)
            .field("missionCompletionTime", TestType::Single)
            .field("missionEndReason", TestType::Locale)
            .field("missionResultAfterTimerEnd", TestType::String)
            .field("remainingTimeToShowTimer", TestType::Single),
    ]
}

fn requirements() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ReputationRequirements"),
        TestStruct::new("SReputationMissionRequirementsParams")
            .parent("ReputationRequirements")
            .array("expression", pointer("ReputationExpression")),
        TestStruct::new("SReputationMissionRequirementsUnknown").parent("ReputationRequirements"),
        TestStruct::new("ReputationExpression"),
        TestStruct::new("SReputationMissionGiverRequirementParams")
            .parent("ReputationExpression")
            .field("comparison", TestType::String)
            .field("factionReputation", TestType::Reference)
            .field("reputationScope", TestType::Reference)
            .field("standing", TestType::Reference),
        TestStruct::new("SReputationMissionRequirementExpression_And")
            .parent("ReputationExpression"),
        TestStruct::new("SReputationMissionRequirementExpression_LeftParenthesis")
            .parent("ReputationExpression"),
        TestStruct::new("SReputationMissionRequirementExpression_RightParenthesis")
            .parent("ReputationExpression"),
        TestStruct::new("ReputationResultSet")
            .array("reputationAmounts", class("ReputationAmountEntry")),
        TestStruct::new("ReputationAmountEntry")
            .field("factionReputation", TestType::Reference)
            .field("reputationScope", TestType::Reference)
            .field("reward", TestType::Reference),
        TestStruct::new("ReputationReward").field("reputationAmount", TestType::Int32),
    ]
}

fn modifiers() -> Vec<TestStruct> {
    vec![
        TestStruct::new("MissionModifier_FactionHostility")
            .parent("MissionModifier")
            .field("modifierName", TestType::String)
            .field("enabled", TestType::Bool)
            .field("faction", TestType::Reference)
            .field("myReaction", TestType::String)
            .field("theirReaction", TestType::String)
            .field("ignoreCriminalHostility", TestType::Bool),
        TestStruct::new("MissionModifier_HostileMission")
            .parent("MissionModifier")
            .field("enabled", TestType::Bool)
            .field("missionBrokerEntry", TestType::Reference)
            .field("legalToAttack", TestType::Bool),
        TestStruct::new("MissionModifier_IgnoreMissionPlayerCriminality")
            .parent("MissionModifier")
            .field("enabled", TestType::Bool)
            .field("missionBrokerEntry", TestType::Reference),
        TestStruct::new("MissionModifier_LawLicense")
            .parent("MissionModifier")
            .field("enabled", TestType::Bool)
            .field("licenseType", TestType::String),
    ]
}
