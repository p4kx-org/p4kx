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
use crate::template_issues::{TemplateError, Unknowns};
use crate::template_objective::ObjectiveState;

/// The deepest nesting of conditions that is followed; a Record that nests deeper cannot be read.
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
    /// A condition of another struct.
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
        /// The `toState`, as for the other actions.
        to_state: Option<ObjectiveState>,
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

/// What the mission flow of one Record is read with: its weak pointers and the count of unknown
/// names.
pub(crate) struct FlowReader<'r, 'x, 'f> {
    pub(crate) reader: &'r Reader<'x, 'f>,
    pub(crate) unknowns: &'r Unknowns,
}

impl FlowReader<'_, '_, '_> {
    /// Reads the `triggers` of the `missionFlow` of `root`.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::ConditionsTooDeep`] for conditions nested deeper than the limit.
    pub(crate) fn triggers(
        &self,
        root: Option<&StructValue<'_>>,
    ) -> Result<Vec<FlowTrigger>, TemplateError> {
        structs(one(root, "missionFlow"), "triggers")
            .map(|trigger| {
                let trigger = Some(trigger);
                Ok(FlowTrigger {
                    note: text(trigger, "description").map(str::to_owned),
                    condition: one(trigger, "condition")
                        .map(|condition| self.condition(condition, 0))
                        .transpose()?,
                    action: one(trigger, "action").map(|action| self.action(action)),
                })
            })
            .collect()
    }

    fn state(&self, parent: Option<&StructValue<'_>>, name: &str) -> Option<ObjectiveState> {
        self.unknowns
            .parse("flow state", text(parent, name), ObjectiveState::parse)
    }

    fn condition(
        &self,
        condition: &StructValue<'_>,
        depth: usize,
    ) -> Result<FlowCondition, TemplateError> {
        if depth > MAX_CONDITION_DEPTH {
            return Err(TemplateError::ConditionsTooDeep(MAX_CONDITION_DEPTH));
        }
        let parent = Some(condition);
        let name = condition.type_name();
        let nested = |field: &str| -> Result<Vec<FlowCondition>, TemplateError> {
            structs(parent, field)
                .map(|inner| self.condition(inner, depth.saturating_add(1)))
                .collect()
        };
        Ok(match name.strip_prefix(CONDITION_PREFIX).unwrap_or(name) {
            "TokenState" => FlowCondition::TokenState {
                objective_id: self.reader.objective_id(parent, "objectiveToken"),
                token_state: self.state(parent, "tokenState"),
            },
            "AllTokensState" => FlowCondition::AllTokensState {
                token_state: self.state(parent, "tokenState"),
            },
            "AND" => FlowCondition::And {
                conditions: nested("conditions")?,
            },
            "OR" => FlowCondition::Or {
                conditions: nested("conditions")?,
            },
            "NOT" => FlowCondition::Not {
                condition: one(parent, "condition")
                    .map(|inner| self.condition(inner, depth.saturating_add(1)).map(Box::new))
                    .transpose()?,
            },
            "PropertyIsSet" => FlowCondition::PropertyIsSet {
                property_variable: self.reader.property_variable(parent, "property"),
            },
            _ => FlowCondition::Other {
                struct_name: name.to_owned(),
            },
        })
    }

    fn action(&self, action: &StructValue<'_>) -> FlowAction {
        let parent = Some(action);
        let name = action.type_name();
        let to_state = self.state(parent, "toState");
        match name.strip_prefix(ACTION_PREFIX).unwrap_or(name) {
            "StateChange" => FlowAction::StateChange { to_state },
            "TokenStateChange" => FlowAction::TokenStateChange {
                objective_id: self.reader.objective_id(parent, "objectiveToken"),
                to_state,
            },
            "PickRandomMissionPhase" => FlowAction::PickRandomMissionPhase {
                phases: structs(parent, "missionPhases")
                    .map(|phase| RandomPhase {
                        objective_id: self.reader.objective_id(Some(phase), "objective"),
                        probability: number(Some(phase), "probability"),
                    })
                    .collect(),
                to_state,
            },
            _ => FlowAction::Other {
                struct_name: name.to_owned(),
                to_state,
            },
        }
    }
}
