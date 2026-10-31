// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The contract template Record of the fixture: display info, class settings, properties and
//! objective tokens.

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::{
    ELSEWHERE_TEMPLATE, LEFTOVER_TEMPLATE, MISSION_TYPE, OBJECTIVE_A, OBJECTIVE_B, RESOURCE,
    TAG_KILL, TEMPLATE, array, location, property, reference, ships, text,
};

pub(super) fn template() -> TestRecord {
    TestRecord::new(
        TEMPLATE,
        "ContractTemplate",
        "ContractTemplate.Hunt",
        "libs/foundry/records/contracts/contracttemplates/hunt.xml",
    )
    .value("notForRelease", TestValue::Bool(false))
    .value("contractDisplayInfo", display_info())
    .value("contractClass", contract_class())
    .value(
        "contractProperties",
        array(vec![
            ships("Wave_BP", "", Some(9)),
            location("Place_BP", "Place", None),
            location("Home_BP", "", None),
            property(
                "Flag_BP",
                "",
                TestInstance::new("MissionPropertyValue_Boolean")
                    .value("value", TestValue::Bool(true)),
            ),
            property(
                "Cargo_BP",
                "Cargo",
                TestInstance::new("MissionPropertyValue_HaulingOrders").value(
                    "haulingOrderContent",
                    array(vec![resource_order(RESOURCE, 4)]),
                ),
            ),
        ]),
    )
    .value(
        "objectiveTokens",
        array(vec![
            objective(
                OBJECTIVE_A,
                "Deliver",
                true,
                Some(resource_order(RESOURCE, 8)),
                vec![objective(OBJECTIVE_B, "", false, None, Vec::new())],
            ),
            TestValue::Struct(TestInstance::new("ObjectiveToken").value("debugName", text("NoId"))),
        ]),
    )
}

fn display_info() -> TestValue {
    TestValue::Struct(
        TestInstance::new("ContractDisplayInfo")
            .value("illegal", TestValue::Bool(true))
            .value("showLifeTimeInMobiGlas", TestValue::Bool(false))
            .value("preShowObjectives", TestValue::Bool(true))
            .value("displayString", array(vec![TestValue::text("@place_name")]))
            .value("type", reference(MISSION_TYPE)),
    )
}

fn contract_class() -> TestValue {
    let settings = TestInstance::new("ActiveContractSettings")
        .value("canBeShared", TestValue::Bool(true))
        .value("hasCompleteButton", TestValue::Bool(false));
    let deadline = TestInstance::new("MissionDeadline")
        .value("missionCompletionTime", TestValue::Int32(300))
        .value("missionAutoEnd", TestValue::Bool(true))
        .value("missionResultAfterTimerEnd", text("Failed"))
        .value("remainingTimeToShowTimer", TestValue::Int32(15));
    let auto_finish = TestInstance::new("ContractAutoFinishSettings")
        .value("failIfSentToPrison", TestValue::Bool(true))
        .value("contractDeadline", TestValue::Struct(deadline));
    TestValue::Struct(
        TestInstance::new("ContractClass_Contract")
            .value("additionalParams", TestValue::Struct(settings))
            .value("autoFinishSettings", TestValue::Struct(auto_finish)),
    )
}

fn resource_order(resource: u128, max_scu: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("HaulingOrder_Resource")
            .value("resource", reference(resource))
            .value("minSCU", TestValue::Int32(1))
            .value("maxSCU", TestValue::Int32(max_scu)),
    )
}

/// An objective token; `hauling` makes it a hauling objective with that order, a token without
/// one is local, and a phase of `phases` is tagged `Kill` when its id is `OBJECTIVE_B`.
fn objective(
    id: u128,
    name: &str,
    starts_active: bool,
    hauling: Option<TestValue>,
    phases: Vec<TestValue>,
) -> TestValue {
    let handler = hauling.map_or_else(
        || TestInstance::new("ObjectiveHandler_Local"),
        |order| {
            TestInstance::new("ObjectiveHandler_Hauling").value("haulingOrders", array(vec![order]))
        },
    );
    let mut token = TestInstance::new("ObjectiveToken")
        .value("id", TestValue::Guid(id))
        .value("debugName", text(name))
        .value("startsActive", TestValue::Bool(starts_active))
        .value("objectiveHandler", TestValue::Struct(handler))
        .value("childMissionPhases", array(phases));
    if id == OBJECTIVE_B {
        token = token.value("missionPhaseIdentifierTag", reference(TAG_KILL));
    }
    TestValue::Struct(token)
}

/// A template whose path marks it as a development leftover.
pub(super) fn leftover() -> TestRecord {
    TestRecord::new(
        LEFTOVER_TEMPLATE,
        "ContractTemplate",
        "ContractTemplate.Dev_Temp",
        "libs/foundry/records/contracts/contracttemplates/dev_temp.xml",
    )
}

/// A template outside the `contracttemplates` folder, whose path holds the word `template`.
pub(super) fn elsewhere() -> TestRecord {
    TestRecord::new(
        ELSEWHERE_TEMPLATE,
        "ContractTemplate",
        "ContractTemplate.Beacon_Template",
        "libs/foundry/records/servicebeacon/beacon_template.xml",
    )
}
