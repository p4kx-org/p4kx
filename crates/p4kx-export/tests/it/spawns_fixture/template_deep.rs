// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Templates that nest their phases or conditions deeper than the catalog follows.

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::template_parts::{display, object, token_state, trigger};
use super::template_values as values;
use super::{
    DEEP_FLOW, DEEP_PHASES, OBJECTIVE_A, UNKNOWN_CONDITION, UNKNOWN_CONTRACT_PROPERTY,
    UNKNOWN_FLOW, UNKNOWN_ITEM_CONDITION, UNKNOWN_LOCATION_CONDITION, UNKNOWN_MODIFIER,
    UNKNOWN_VALUE, array, npc, text,
};

/// The levels of phases the deep template nests, more than the catalog follows.
const DEPTH_PAST_PHASES: usize = 35;

/// The `NOT`s the deep flow nests, more than the catalog follows.
const CONDITIONS_PAST_LIMIT: usize = 18;

/// A chain of `levels` objective tokens, each the only phase of the one above it.
fn phase_chain(levels: usize) -> TestValue {
    let chain = (0..levels).rev().fold(None, |child, level| {
        let id = DEEP_PHASES.saturating_add(u128::try_from(level).unwrap_or_default());
        let token = TestInstance::new("ObjectiveToken")
            .value("id", TestValue::Guid(id))
            .value("displayInfo", display("@LOC_UNINITIALIZED"));
        Some(match child {
            Some(phase) => token.value("childMissionPhases", array(vec![object(phase)])),
            None => token,
        })
    });
    chain.map_or(TestValue::Null, object)
}

/// A template that nests its objective phases past what the catalog follows.
pub(super) fn deep_phases() -> TestRecord {
    TestRecord::new(
        DEEP_PHASES,
        "ContractTemplate",
        "ContractTemplate.Deep_Phases",
        "libs/foundry/records/contracts/contracttemplates/deep_phases.xml",
    )
    .value(
        "objectiveTokens",
        array(vec![phase_chain(DEPTH_PAST_PHASES)]),
    )
}

/// A template whose mission flow holds a condition of a struct the catalog does not know.
pub(super) fn unknown_flow() -> TestRecord {
    let trigger = trigger(
        "Odd",
        TestInstance::new("MissionFlowCondition_Unknown"),
        TestInstance::new("MissionFlowAction_StateChange").value("toState", text("Failed")),
    );
    TestRecord::new(
        UNKNOWN_FLOW,
        "ContractTemplate",
        "ContractTemplate.Unknown_Flow",
        "libs/foundry/records/contracts/contracttemplates/unknown_flow.xml",
    )
    .value(
        "missionFlow",
        object(TestInstance::new("MissionFlow").value("triggers", array(vec![trigger]))),
    )
}

/// A template with a modifier of a struct the catalog does not know.
pub(super) fn unknown_modifier() -> TestRecord {
    TestRecord::new(
        UNKNOWN_MODIFIER,
        "ContractTemplate",
        "ContractTemplate.Unknown_Modifier",
        "libs/foundry/records/contracts/contracttemplates/unknown_modifier.xml",
    )
    .value(
        "modifiers",
        array(vec![object(TestInstance::new("MissionModifier"))]),
    )
}

/// An objective token with the embedded property `value`.
fn token_with(value: TestValue) -> TestValue {
    object(
        TestInstance::new("ObjectiveToken")
            .value("id", TestValue::Guid(OBJECTIVE_A))
            .value("displayInfo", display("@LOC_UNINITIALIZED"))
            .value(
                "properties",
                array(vec![object(
                    TestInstance::new("ObjectiveProperty_Embedded").value("property", value),
                )]),
            ),
    )
}

/// A template whose objective holds a property value of a struct the catalog does not know.
pub(super) fn unknown_value() -> TestRecord {
    TestRecord::new(
        UNKNOWN_VALUE,
        "ContractTemplate",
        "ContractTemplate.Unknown_Value",
        "libs/foundry/records/contracts/contracttemplates/unknown_value.xml",
    )
    .value(
        "objectiveTokens",
        array(vec![token_with(npc::mystery_value("Item_BP"))]),
    )
}

/// A template with a contract property of a struct the catalog does not know.
pub(super) fn unknown_contract_property() -> TestRecord {
    TestRecord::new(
        UNKNOWN_CONTRACT_PROPERTY,
        "ContractTemplate",
        "ContractTemplate.Unknown_Contract_Property",
        "libs/foundry/records/contracts/contracttemplates/unknown_contract_property.xml",
    )
    .value(
        "contractProperties",
        array(vec![npc::mystery_value("Mystery_BP")]),
    )
}

/// A template with a contract property of a mission item whose condition is of an unknown struct.
pub(super) fn unknown_item_condition() -> TestRecord {
    TestRecord::new(
        UNKNOWN_ITEM_CONDITION,
        "ContractTemplate",
        "ContractTemplate.Unknown_Item_Condition",
        "libs/foundry/records/contracts/contracttemplates/unknown_item_condition.xml",
    )
    .value(
        "contractProperties",
        array(vec![values::mystery_item("Item_BP")]),
    )
}

/// A template with a contract property of a location query whose condition is of an unknown
/// struct.
pub(super) fn unknown_location_condition() -> TestRecord {
    TestRecord::new(
        UNKNOWN_LOCATION_CONDITION,
        "ContractTemplate",
        "ContractTemplate.Unknown_Location_Condition",
        "libs/foundry/records/contracts/contracttemplates/unknown_location_condition.xml",
    )
    .value(
        "contractProperties",
        array(vec![values::mystery_location("Place_BP")]),
    )
}

/// A template whose objective holds an organisation with a condition of an unknown struct.
pub(super) fn unknown_condition() -> TestRecord {
    TestRecord::new(
        UNKNOWN_CONDITION,
        "ContractTemplate",
        "ContractTemplate.Unknown_Condition",
        "libs/foundry/records/contracts/contracttemplates/unknown_condition.xml",
    )
    .value(
        "objectiveTokens",
        array(vec![token_with(npc::organization("Giver_BP", false))]),
    )
}

/// A token state condition inside `levels` nested `NOT`s.
fn condition_chain(levels: usize) -> TestInstance {
    let mut condition = token_state("Active", None);
    for _ in 0..levels {
        condition =
            TestInstance::new("MissionFlowCondition_NOT").value("condition", object(condition));
    }
    condition
}

/// A template whose mission flow nests its conditions past what the catalog follows.
pub(super) fn deep_flow() -> TestRecord {
    let trigger = trigger(
        "Deep",
        condition_chain(CONDITIONS_PAST_LIMIT),
        TestInstance::new("MissionFlowAction_StateChange").value("toState", text("Failed")),
    );
    TestRecord::new(
        DEEP_FLOW,
        "ContractTemplate",
        "ContractTemplate.Deep_Flow",
        "libs/foundry/records/contracts/contracttemplates/deep_flow.xml",
    )
    .value(
        "missionFlow",
        object(TestInstance::new("MissionFlow").value("triggers", array(vec![trigger]))),
    )
}
