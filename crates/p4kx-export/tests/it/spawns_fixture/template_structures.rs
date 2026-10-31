// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The structs of the objective tokens, the mission flow, the modifiers, the payouts and the
//! comms of the contract template Records of the fixture.

use p4kx_testkit::{TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

/// Returns the structs of the fixture's template parts.
pub(super) fn structs() -> Vec<TestStruct> {
    [objectives(), handlers(), flow(), extras()]
        .into_iter()
        .flatten()
        .collect()
}

fn objectives() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ObjectiveToken")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .field("startsActive", TestType::Bool)
            .field("variableName", TestType::String)
            .field("weight", TestType::Single)
            .field("objectiveHandler", pointer("ObjectiveHandler"))
            .field("missionPhaseIdentifierTag", TestType::Reference)
            .field("displayInfo", class("ObjectiveDisplayInfo"))
            .field(
                "overrideMissionDetailsDisplayInfo",
                class("OverrideMissionDetailsDisplayInfo"),
            )
            .field("rewardContribution", pointer("ObjectiveRewardContribution"))
            .array("commsNotifications", class("CommsNotificationSelector"))
            .array("childMissionPhases", class("ObjectiveToken")),
        TestStruct::new("ObjectiveDisplayInfo")
            .field("category", TestType::String)
            .field("hideOnHUD", TestType::Bool)
            .field("objectiveMarkerLabel", TestType::Locale)
            .field("shortDescription", TestType::Locale)
            .field("longDescription", TestType::Locale),
        TestStruct::new("OverrideMissionDetailsDisplayInfo")
            .field("titleOverride", TestType::Locale)
            .field("descriptionOverride", TestType::Locale),
        TestStruct::new("ObjectiveRewardContribution"),
        TestStruct::new("ObjectiveRewardContribution_PercentOfMission")
            .parent("ObjectiveRewardContribution")
            .field("percentContribution", TestType::Int32)
            .field("doPartialRewardPayout", TestType::Bool),
        TestStruct::new("CommsNotificationSelector")
            .field("trigger", TestType::String)
            .field("commsTiming", TestType::String)
            .field("delay", TestType::Int32)
            .field("communicationTags", TestType::Reference),
    ]
}

fn handlers() -> Vec<TestStruct> {
    vec![
        TestStruct::new("ObjectiveHandler"),
        TestStruct::new("ObjectiveHandler_Local")
            .parent("ObjectiveHandler")
            .field("module", TestType::String)
            .field("travelRadiusKM", TestType::Single)
            .field("disableTravelObjectives", TestType::Bool)
            .field("travelObjectiveInfo", class("ObjectiveDisplayInfo"))
            .field("navPointSpawnInfo", pointer("NavPointSpawnInformation")),
        TestStruct::new("NavPointSpawnInformation")
            .field("entityClassDefinition", TestType::Reference)
            .field("name", TestType::Locale)
            .array("extendedTextTokens", TestType::String),
        TestStruct::new("ObjectiveHandler_TransitToInstance")
            .parent("ObjectiveHandler")
            .field("completePhaseOnPlayerEnterInstance", TestType::Bool)
            .field("instanceEnterTravelRadiusKM", TestType::Single)
            .field("instanceHUDObjectiveText", TestType::Locale)
            .field("instanceTagIdentifier", TestType::Reference)
            .array("ocTagsToSearchInstanceEnter", TestType::Reference)
            .field("transitToInstanceObjectiveMarker", TestType::String),
        TestStruct::new("ObjectiveHandler_Hauling")
            .parent("ObjectiveHandler")
            .field("hideMarkers", TestType::Bool)
            .field("objectiveSettings", class("Hauling_ObjectiveSettings"))
            .array("haulingOrders", pointer("HaulingOrder")),
        TestStruct::new("Hauling_ObjectiveSettings")
            .field("itemExtendedTextToken", TestType::String)
            .field("pickupLocationExtendedTextToken", TestType::String)
            .field("amountExtendedTextToken", TestType::String)
            .field("totalExtendedTextToken", TestType::String)
            .field("dropOffLocationExtendedTextToken", TestType::String)
            .field("andObjective", class("ObjectiveDisplayInfo"))
            .field("collectObjective", class("ObjectiveDisplayInfo")),
        TestStruct::new("HaulingOrder"),
    ]
}

fn flow() -> Vec<TestStruct> {
    vec![
        TestStruct::new("MissionFlow").array("triggers", class("MissionFlowTrigger")),
        TestStruct::new("MissionFlowTrigger")
            .field("description", TestType::String)
            .field("condition", pointer("MissionFlowCondition"))
            .field("action", pointer("MissionFlowAction")),
        TestStruct::new("MissionFlowCondition"),
        TestStruct::new("MissionFlowCondition_TokenState")
            .parent("MissionFlowCondition")
            .field("objectiveToken", TestType::Reference)
            .field("tokenState", TestType::String),
        TestStruct::new("MissionFlowCondition_AND")
            .parent("MissionFlowCondition")
            .array("conditions", pointer("MissionFlowCondition")),
        TestStruct::new("MissionFlowCondition_NOT")
            .parent("MissionFlowCondition")
            .field("condition", pointer("MissionFlowCondition")),
        TestStruct::new("MissionFlowCondition_Unknown").parent("MissionFlowCondition"),
        TestStruct::new("MissionFlowAction"),
        TestStruct::new("MissionFlowAction_StateChange")
            .parent("MissionFlowAction")
            .field("toState", TestType::String),
        TestStruct::new("MissionFlowAction_PickRandomMissionPhase")
            .parent("MissionFlowAction")
            .field("toState", TestType::String)
            .array("missionPhases", class("MissionRandomPhaseEntry")),
        TestStruct::new("MissionRandomPhaseEntry")
            .field("objective", TestType::Reference)
            .field("probability", TestType::Single),
    ]
}

fn extras() -> Vec<TestStruct> {
    vec![
        TestStruct::new("DevOwner"),
        TestStruct::new("PartialContractRewards")
            .array("percentageThresholds", class("PartialContractRewardRange")),
        TestStruct::new("PartialContractRewardRange")
            .field("minPercentage", TestType::Int32)
            .field("maxPercentage", TestType::Int32)
            .field("currencyRewardMultiplier", TestType::Single)
            .array(
                "reputationMultipliers",
                class("PartialContractRewardRepAdjustment"),
            ),
        TestStruct::new("PartialContractRewardRepAdjustment")
            .field("reputationScope", TestType::Reference)
            .field("reputationRewardMultiplier", TestType::Int32),
        TestStruct::new("MissionModifier"),
        TestStruct::new("MissionModifier_SecurityClearance")
            .parent("MissionModifier")
            .field("modifierName", TestType::String)
            .field("enabled", TestType::Bool)
            .field("clearanceToken", TestType::Reference),
        TestStruct::new("MissionModifier_RequestRentShip")
            .parent("MissionModifier")
            .field("modifierName", TestType::String)
            .field("enabled", TestType::Bool)
            .field("itemRecordGUID", TestType::Reference)
            .field("durationSeconds", TestType::Int32)
            .field("clearRentalOnFail", TestType::Bool),
        TestStruct::new("MissionModifier_ShipKioskFilter")
            .parent("MissionModifier")
            .field("enabled", TestType::Bool)
            .field("kioskFilterType", class("TagsDNF")),
        TestStruct::new("TagsDNF").array("terms", class("TagsDNFTerm")),
        TestStruct::new("TagsDNFTerm")
            .array("positiveTags", TestType::Reference)
            .array("negativeTags", TestType::Reference),
        TestStruct::new("ContractCommsNotification")
            .field("delay", TestType::Int32)
            .field("communicationTags", TestType::Reference),
        TestStruct::new("ContractEndCommsNotification")
            .field("reason", TestType::String)
            .field("delay", TestType::Int32)
            .field("communicationTags", TestType::Reference),
    ]
}
