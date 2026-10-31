// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::mission_layers::one;
use crate::record_fields::{number, structs, text};
use crate::spawn_read::Reader;
use crate::template_objective::ObjectiveState;

/// The deepest nesting of conditions that is followed; the Records of LIVE nest three levels. A
/// deeper condition is stated as `other`.
const MAX_CONDITION_DEPTH: usize = 16;

const CONDITION_PREFIX: &str = "MissionFlowCondition_";
const ACTION_PREFIX: &str = "MissionFlowAction_";

/// A trigger of the mission flow: when its condition holds, its action runs.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct FlowTrigger {
    /// The `description` as stored, a developer's note on the trigger; `null` when empty.
    pub(crate) note: Option<String>,
    /// The `condition`, `null` when absent.
    pub(crate) condition: Option<FlowCondition>,
    /// The `action`, `null` when absent.
    pub(crate) action: Option<FlowAction>,
}

/// A condition of a mission flow trigger (`MissionFlowCondition_*`), as stored. An objective
/// is the `id` of an entry of `objectives`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum FlowCondition {
    /// `MissionFlowCondition_TokenState`: an objective is in a state.
    #[serde(rename_all = "camelCase")]
    TokenState {
        /// The `objectiveToken`, `null` when it points to none.
        objective_id: Option<CatalogGuid>,
        /// The `tokenState`.
        token_state: Option<ObjectiveState>,
    },
    /// `MissionFlowCondition_AllTokensState`: every objective is in a state.
    #[serde(rename_all = "camelCase")]
    AllTokensState {
        /// The `tokenState`.
        token_state: Option<ObjectiveState>,
    },
    /// `MissionFlowCondition_AND`: all of the conditions hold.
    And {
        /// The `conditions` in stored order.
        conditions: Vec<Self>,
    },
    /// `MissionFlowCondition_OR`: one of the conditions holds.
    Or {
        /// The `conditions` in stored order.
        conditions: Vec<Self>,
    },
    /// `MissionFlowCondition_NOT`: the condition does not hold.
    Not {
        /// The `condition`, `null` when absent.
        condition: Option<Box<Self>>,
    },
    /// `MissionFlowCondition_PropertyIsSet`: a mission property has a value.
    #[serde(rename_all = "camelCase")]
    PropertyIsSet {
        /// The `missionVariableName` of the `property`.
        property_variable: Option<String>,
    },
    /// A condition of another struct, or one nested deeper than the catalog follows.
    #[serde(rename_all = "camelCase")]
    Other {
        /// The struct name.
        struct_name: String,
    },
}

/// An action of a mission flow trigger (`MissionFlowAction_*`), as stored.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum FlowAction {
    /// `MissionFlowAction_StateChange`: the mission changes state.
    #[serde(rename_all = "camelCase")]
    StateChange {
        /// The `toState`.
        to_state: Option<ObjectiveState>,
    },
    /// `MissionFlowAction_TokenStateChange`: an objective changes state.
    #[serde(rename_all = "camelCase")]
    TokenStateChange {
        /// The `objectiveToken`, `null` when it points to none.
        objective_id: Option<CatalogGuid>,
        /// The `toState`.
        to_state: Option<ObjectiveState>,
    },
    /// `MissionFlowAction_PickRandomMissionPhase`: one of the phases becomes active.
    #[serde(rename_all = "camelCase")]
    PickRandomMissionPhase {
        /// The `missionPhases` in stored order.
        phases: Vec<RandomPhase>,
        /// The `toState`.
        to_state: Option<ObjectiveState>,
    },
    /// An action of another struct.
    #[serde(rename_all = "camelCase")]
    Other {
        /// The struct name.
        struct_name: String,
    },
}

/// A `MissionRandomPhaseEntry`.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RandomPhase {
    /// The `objective`, an entry of `objectives`; `null` when it points to none.
    pub(crate) objective_id: Option<CatalogGuid>,
    /// The `probability`, as stored.
    pub(crate) probability: Option<f32>,
}

/// Reads the `triggers` of the `missionFlow` of the Record `reader` belongs to.
pub(crate) fn triggers(
    reader: &Reader<'_, '_>,
    root: Option<&StructValue<'_>>,
) -> Vec<FlowTrigger> {
    structs(one(root, "missionFlow"), "triggers")
        .map(|trigger| {
            let trigger = Some(trigger);
            FlowTrigger {
                note: text(trigger, "description").map(str::to_owned),
                condition: one(trigger, "condition")
                    .map(|condition| read_condition(reader, condition, 0)),
                action: one(trigger, "action").map(|action| read_action(reader, action)),
            }
        })
        .collect()
}

fn state(parent: Option<&StructValue<'_>>, name: &str) -> Option<ObjectiveState> {
    text(parent, name).and_then(ObjectiveState::parse)
}

fn read_condition(
    reader: &Reader<'_, '_>,
    condition: &StructValue<'_>,
    depth: usize,
) -> FlowCondition {
    let parent = Some(condition);
    let name = condition.type_name();
    let nested = |field: &str| -> Vec<FlowCondition> {
        structs(parent, field)
            .map(|inner| read_condition(reader, inner, depth.saturating_add(1)))
            .collect()
    };
    if depth >= MAX_CONDITION_DEPTH {
        return FlowCondition::Other {
            struct_name: name.to_owned(),
        };
    }
    match name.strip_prefix(CONDITION_PREFIX).unwrap_or(name) {
        "TokenState" => FlowCondition::TokenState {
            objective_id: reader.objective_id(parent, "objectiveToken"),
            token_state: state(parent, "tokenState"),
        },
        "AllTokensState" => FlowCondition::AllTokensState {
            token_state: state(parent, "tokenState"),
        },
        "AND" => FlowCondition::And {
            conditions: nested("conditions"),
        },
        "OR" => FlowCondition::Or {
            conditions: nested("conditions"),
        },
        "NOT" => FlowCondition::Not {
            condition: one(parent, "condition")
                .map(|inner| Box::new(read_condition(reader, inner, depth.saturating_add(1)))),
        },
        "PropertyIsSet" => FlowCondition::PropertyIsSet {
            property_variable: reader.property_variable(parent, "property"),
        },
        _ => FlowCondition::Other {
            struct_name: name.to_owned(),
        },
    }
}

fn read_action(reader: &Reader<'_, '_>, action: &StructValue<'_>) -> FlowAction {
    let parent = Some(action);
    let name = action.type_name();
    match name.strip_prefix(ACTION_PREFIX).unwrap_or(name) {
        "StateChange" => FlowAction::StateChange {
            to_state: state(parent, "toState"),
        },
        "TokenStateChange" => FlowAction::TokenStateChange {
            objective_id: reader.objective_id(parent, "objectiveToken"),
            to_state: state(parent, "toState"),
        },
        "PickRandomMissionPhase" => FlowAction::PickRandomMissionPhase {
            phases: structs(parent, "missionPhases")
                .map(|phase| RandomPhase {
                    objective_id: reader.objective_id(Some(phase), "objective"),
                    probability: number(Some(phase), "probability"),
                })
                .collect(),
            to_state: state(parent, "toState"),
        },
        _ => FlowAction::Other {
            struct_name: name.to_owned(),
        },
    }
}
