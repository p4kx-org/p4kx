// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_source_meaning::{ObjectiveBinding, StateInterpretation};
use crate::template_source_refs::FlowFieldBinding;

/// An action interpretation tied to its actual original input fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum ActionMeaning {
    StateChange {
        to_state_field: FlowFieldBinding,
        state: StateInterpretation,
    },
    TokenStateChange {
        objective_token_field: FlowFieldBinding,
        objective: ObjectiveBinding,
        to_state_field: FlowFieldBinding,
        state: StateInterpretation,
    },
    PickRandomMissionPhase {
        to_state_field: FlowFieldBinding,
        state: StateInterpretation,
        mission_phases_field: FlowFieldBinding,
        phases: Vec<SourceValueRef>,
    },
    Unsupported,
}

/// The established finite-f32 recipe result without a second approximate numeric datum.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum ProbabilityInterpretation {
    Finite,
    Nonfinite,
    OutOfRange,
    Absent,
    Null,
    WrongShape,
    Unreadable,
}

/// One actual random-phase entry's bindings into the canonical source inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct RandomPhaseMeaning {
    pub(crate) objective_field: FlowFieldBinding,
    pub(crate) objective: ObjectiveBinding,
    pub(crate) probability_field: FlowFieldBinding,
    pub(crate) probability_interpretation: ProbabilityInterpretation,
}
