// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The contract template Record of the fixture: display info, class settings, properties and
//! objective tokens.

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::template_parts as parts;
use super::template_properties as properties;
use super::{
    ELSEWHERE_TEMPLATE, LEFTOVER_TEMPLATE, MISSION_TYPE, OBJECTIVE_A, OBJECTIVE_B, OBJECTIVE_C,
    OWNER, RESOURCE, TAG_KILL, TEMPLATE, array, location, property, reference, ships, text,
};
/// The template the fixture's contract names. The targets of the weak pointers (the properties
/// and the objective tokens) are written before the values that point to them.
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
            parts::named(location("Place_BP", "Place", None), "prop_place"),
            parts::named(location("Home_BP", "", None), "prop_home"),
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
    .value("objectiveTokens", objective_tokens())
    .value("owner", reference(OWNER))
    .value("partialRewardPayout", parts::payout())
    .value("modifiers", parts::modifiers())
    .value("missionFlow", parts::flow())
    .value("contractStartCommsNotification", parts::start_comms())
    .value("contractEndCommsNotifications", parts::end_comms())
}

/// Three objectives (a hauling one with an instance phase below it, and a local one) and a token
/// without an id.
fn objective_tokens() -> TestValue {
    let phase = objective(
        (OBJECTIVE_B, "tok_b"),
        ("", false),
        parts::instance_handler(),
        (vec![properties::input_property()], Vec::new()),
    );
    array(vec![
        objective(
            (OBJECTIVE_A, "tok_a"),
            ("Deliver", true),
            parts::hauling_handler(resource_order(RESOURCE, 8)),
            (properties::properties(), vec![phase]),
        ),
        objective(
            (OBJECTIVE_C, "tok_c"),
            ("Go", true),
            parts::local_handler(),
            (Vec::new(), Vec::new()),
        ),
        TestValue::Struct(
            TestInstance::new("ObjectiveToken")
                .named("tok_noid")
                .value("debugName", text("NoId")),
        ),
    ])
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

/// An objective token named `instance` for the weak pointers, with the handler `handler`, the
/// properties and the phases; a phase with the id `OBJECTIVE_B` is tagged `Kill`.
fn objective(
    (id, instance): (u128, &str),
    (name, starts_active): (&str, bool),
    handler: TestInstance,
    (properties, phases): (Vec<TestValue>, Vec<TestValue>),
) -> TestValue {
    let mut token = TestInstance::new("ObjectiveToken")
        .named(instance)
        .value("id", TestValue::Guid(id))
        .value("debugName", text(name))
        .value("startsActive", TestValue::Bool(starts_active))
        .value("objectiveHandler", TestValue::Struct(handler))
        .value("properties", array(properties))
        .value("childMissionPhases", array(phases));
    if id == OBJECTIVE_B {
        token = token.value("missionPhaseIdentifierTag", reference(TAG_KILL));
    }
    TestValue::Struct(parts::dress(token, id))
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
