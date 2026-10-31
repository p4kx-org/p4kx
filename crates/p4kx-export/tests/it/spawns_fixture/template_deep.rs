// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Templates that nest their phases or conditions deeper than the catalog follows.

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::template_parts::{display, object, token_state, trigger};
use super::{DEEP_FLOW, DEEP_PHASES, array, text};

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
