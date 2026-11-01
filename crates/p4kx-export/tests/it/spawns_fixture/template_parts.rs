// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The owner, payout, modifiers, mission flow, comms and objective texts of the fixture's
//! contract template.

use super::{OBJECTIVE_A, OBJECTIVE_B, OWNER, SCOPE, TAG_KILL, array, reference, text};
use p4kx_testkit::{TestInstance, TestRecord, TestValue};

pub(super) fn locale(key: &str) -> TestValue {
    TestValue::text(key)
}

pub(super) const fn object(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}

/// The `DevOwner` Record the template names.
pub(super) fn owner() -> TestRecord {
    TestRecord::new(
        OWNER,
        "DevOwner",
        "DevOwner.MDT",
        "libs/foundry/records/devowners/devteams/mdt.xml",
    )
}

/// A payout with one range that pays 0.76 of the currency and 2 times the reputation of a scope.
pub(super) fn payout() -> TestValue {
    object(TestInstance::new("PartialContractRewards").value(
        "percentageThresholds",
        array(vec![object(
            TestInstance::new("PartialContractRewardRange")
                .value("minPercentage", TestValue::Int32(76))
                .value("maxPercentage", TestValue::Int32(99))
                .value("currencyRewardMultiplier", TestValue::Single(0.76))
                .value(
                    "reputationMultipliers",
                    array(vec![object(
                        TestInstance::new("PartialContractRewardRepAdjustment")
                            .value("reputationScope", reference(SCOPE))
                            .value("reputationRewardMultiplier", TestValue::Int32(2)),
                    )]),
                ),
        )]),
    ))
}

/// A security clearance, a ship rental, a kiosk filter and a law licence.
pub(super) fn modifiers() -> TestValue {
    array(vec![
        object(
            TestInstance::new("MissionModifier_SecurityClearance")
                .value("modifierName", text("GiveClearence"))
                .value("locationProperty", TestValue::pointer("prop_home"))
                .value("enabled", TestValue::Bool(true))
                .value("clearanceToken", reference(OWNER)),
        ),
        object(
            TestInstance::new("MissionModifier_RequestRentShip")
                .value("modifierName", text("RentalMiner"))
                .value("enabled", TestValue::Bool(true))
                .value("durationSeconds", TestValue::Int32(3600))
                .value("clearRentalOnFail", TestValue::Bool(true)),
        ),
        object(
            TestInstance::new("MissionModifier_ShipKioskFilter")
                .value("enabled", TestValue::Bool(false))
                .value(
                    "kioskFilterType",
                    object(TestInstance::new("TagsDNF").value(
                        "terms",
                        array(vec![object(
                            TestInstance::new("TagsDNFTerm")
                                .value("positiveTags", array(vec![reference(TAG_KILL)])),
                        )]),
                    )),
                ),
        ),
        object(
            TestInstance::new("MissionModifier_LawLicense")
                .value("enabled", TestValue::Bool(true))
                .value("licenseType", text("IllegalGoodsLicense_ClassA")),
        ),
    ])
}

pub(super) fn trigger(
    description: &str,
    condition: TestInstance,
    action: TestInstance,
) -> TestValue {
    object(
        TestInstance::new("MissionFlowTrigger")
            .value("description", text(description))
            .value("condition", object(condition))
            .value("action", object(action)),
    )
}

/// A token state condition on the objective token `target`, a weak pointer; none leaves it null.
pub(super) fn token_state(state: &str, target: Option<&str>) -> TestInstance {
    let condition =
        TestInstance::new("MissionFlowCondition_TokenState").value("tokenState", text(state));
    match target {
        Some(name) => condition.value("objectiveToken", TestValue::pointer(name)),
        None => condition,
    }
}

/// Eight triggers: a nested condition with a state change, a random phase pick, a condition on all
/// tokens, one on a token without an id, two that point to a property and to a token the Record
/// lacks, and a condition on any token and one on the start of the mission.
pub(super) fn flow() -> TestValue {
    let nested = TestInstance::new("MissionFlowCondition_AND").value(
        "conditions",
        array(vec![
            object(token_state("Completed", Some("tok_a"))),
            object(
                TestInstance::new("MissionFlowCondition_NOT")
                    .value("condition", object(token_state("Failed", Some("tok_b")))),
            ),
        ]),
    );
    let pick = TestInstance::new("MissionFlowAction_PickRandomMissionPhase")
        .value("toState", text("Active"))
        .value(
            "missionPhases",
            array(vec![object(
                TestInstance::new("MissionRandomPhaseEntry")
                    .value("objective", TestValue::pointer("tok_c"))
                    .value("probability", TestValue::Single(0.25)),
            )]),
        );
    let state_change =
        |to: &str| TestInstance::new("MissionFlowAction_StateChange").value("toState", text(to));
    let property_set = TestInstance::new("MissionFlowCondition_PropertyIsSet")
        .value("property", TestValue::pointer("prop_place"));
    object(TestInstance::new("MissionFlow").value(
        "triggers",
        array(vec![
            trigger("All done -> Completed", nested, state_change("Completed")),
            trigger("Pick", token_state("Active", None), pick),
            trigger(
                "Odd",
                TestInstance::new("MissionFlowCondition_AllTokensState")
                    .value("tokenState", text("Completed")),
                state_change("Sleeping"),
            ),
            trigger(
                "No id",
                token_state("Failed", Some("tok_noid")),
                state_change("Failed"),
            ),
            trigger("Property", property_set, state_change("Active")),
            trigger(
                "Outside",
                token_state("Active", Some("outside")),
                state_change("Active"),
            ),
            trigger(
                "Any",
                TestInstance::new("MissionFlowCondition_AnyTokensState")
                    .value("tokenState", text("Failed")),
                state_change("Active"),
            ),
            trigger(
                "Start",
                TestInstance::new("MissionFlowCondition_OnMissionStart"),
                state_change("Active"),
            ),
        ]),
    ))
}

/// Gives the instance `value` creates the name `name`, so a weak pointer can point to it.
pub(super) fn named(value: TestValue, name: &str) -> TestValue {
    match value {
        TestValue::Struct(instance) => TestValue::Struct(instance.named(name)),
        other => other,
    }
}

/// The start message of the template.
pub(super) fn start_comms() -> TestValue {
    object(
        TestInstance::new("ContractCommsNotification")
            .value("delay", TestValue::Int32(3))
            .value("communicationTags", reference(TAG_KILL)),
    )
}

/// An end message for a completed and one for an unknown reason.
pub(super) fn end_comms() -> TestValue {
    array(vec![
        object(
            TestInstance::new("ContractEndCommsNotification")
                .value("reason", text("Complete"))
                .value("delay", TestValue::Int32(5))
                .value("communicationTags", reference(TAG_KILL)),
        ),
        object(TestInstance::new("ContractEndCommsNotification").value("reason", text("Vanish"))),
    ])
}

pub(super) fn display(short: &str) -> TestValue {
    object(
        TestInstance::new("ObjectiveDisplayInfo")
            .value("category", text("Primary"))
            .value("hideOnHUD", TestValue::Bool(false))
            .value("objectiveMarkerLabel", locale("@LOC_UNINITIALIZED"))
            .value("shortDescription", locale(short))
            .value("longDescription", locale("@place_name")),
    )
}

/// The handler of a hauling objective: markers and the texts of two of its phrasings.
pub(super) fn hauling_handler(order: TestValue) -> TestInstance {
    TestInstance::new("ObjectiveHandler_Hauling")
        .value("hideMarkers", TestValue::Bool(true))
        .value("haulingOrders", array(vec![order]))
        .value(
            "objectiveSettings",
            object(
                TestInstance::new("Hauling_ObjectiveSettings")
                    .value("itemExtendedTextToken", text("item"))
                    .value("dropOffLocationExtendedTextToken", text("destination"))
                    .value("andObjective", display("@place_name"))
                    .value("collectObjective", display("@LOC_UNINITIALIZED")),
            ),
        )
}

/// The handler of a local objective: a radius, a travel text and a marker entity.
pub(super) fn local_handler() -> TestInstance {
    TestInstance::new("ObjectiveHandler_Local")
        .value("module", text("Libs/Subsumption/Missions/Goto.xml"))
        .value("travelRadiusKM", TestValue::Single(2.5))
        .value("disableTravelObjectives", TestValue::Bool(false))
        .value("location", TestValue::pointer("prop_place"))
        .value("travelObjectiveInfo", display("@place_name"))
        .value(
            "navPointSpawnInfo",
            object(
                TestInstance::new("NavPointSpawnInformation")
                    .value("entityClassDefinition", reference(OWNER))
                    .value("name", locale("@place_name")),
            ),
        )
}

/// The handler that transits to an instance.
pub(super) fn instance_handler() -> TestInstance {
    TestInstance::new("ObjectiveHandler_TransitToInstance")
        .value("completePhaseOnPlayerEnterInstance", TestValue::Bool(true))
        .value("instanceEnterTravelRadiusKM", TestValue::Single(4.0))
        .value("instanceHUDObjectiveText", locale("@place_name"))
        .value("instanceTagIdentifier", reference(TAG_KILL))
        .value(
            "ocTagsToSearchInstanceEnter",
            array(vec![reference(TAG_KILL)]),
        )
        .value("transitToInstanceObjectiveMarker", text("Marker"))
}

/// Adds the texts, the reward contribution and the comms of the fixture to the objective `id`.
pub(super) fn dress(token: TestInstance, id: u128) -> TestInstance {
    let token = token.value("displayInfo", display("@place_name")).value(
        "overrideMissionDetailsDisplayInfo",
        object(
            TestInstance::new("OverrideMissionDetailsDisplayInfo")
                .value("titleOverride", locale("@place_name"))
                .value("descriptionOverride", locale("@LOC_UNINITIALIZED")),
        ),
    );
    if id == OBJECTIVE_A {
        token
            .value("variableName", text(""))
            .value(
                "rewardContribution",
                object(
                    TestInstance::new("ObjectiveRewardContribution_PercentOfMission")
                        .value("percentContribution", TestValue::Int32(60))
                        .value("doPartialRewardPayout", TestValue::Bool(true)),
                ),
            )
            .value(
                "commsNotifications",
                array(vec![object(
                    TestInstance::new("CommsNotificationSelector")
                        .value("trigger", text("Completed"))
                        .value("commsTiming", text("HUDNotificationFirst"))
                        .value("delay", TestValue::Int32(2))
                        .value("communicationTags", reference(TAG_KILL)),
                )]),
            )
    } else if id == OBJECTIVE_B {
        token
            .value("variableName", text("PhaseB"))
            .value("weight", TestValue::Single(0.5))
    } else {
        token
            .value("variableName", text(""))
            .value("missionPhaseIdentifierTag", reference(0))
    }
}
