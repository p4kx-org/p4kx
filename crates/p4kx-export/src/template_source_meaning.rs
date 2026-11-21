// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::FieldOrigin;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_objective::ObjectiveState;
use crate::template_source_refs::{FlowFieldBinding, SourceFieldRef};

/// Interpretation of the original selected objective-id field.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum ObjectiveInterpretation {
    Id,
    Nil,
    Absent,
    Null,
    WrongShape,
    Unreadable,
    Unresolved,
    OutsideOwner,
    RecordTarget,
    WrongStorage,
}

/// Interpretation of the original selected property-variable field.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum PropertyInterpretation {
    Value,
    Empty,
    Absent,
    Null,
    WrongShape,
    Unreadable,
    Unresolved,
    OutsideOwner,
    RecordTarget,
    WrongStorage,
}

/// A contextual relationship whose raw id remains in its one original cell.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ObjectiveBinding {
    pub(crate) field: Option<SourceFieldRef>,
    pub(crate) interpretation: ObjectiveInterpretation,
    #[serde(flatten)]
    pub(crate) origin: FieldOrigin,
}

/// A contextual relationship whose raw variable remains in its one original cell.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PropertyBinding {
    pub(crate) field: Option<SourceFieldRef>,
    pub(crate) interpretation: PropertyInterpretation,
    #[serde(flatten)]
    pub(crate) origin: FieldOrigin,
}

/// The known vocabulary result separately from the original stored label.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum StateInterpretation {
    Known { state: ObjectiveState },
    Unknown,
    Absent,
    Null,
    WrongShape,
    Unreadable,
}

/// The original note's interpretation without copying or expanding its source text.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum NoteInterpretation {
    Value,
    Empty,
    Absent,
    Null,
    WrongShape,
    Unreadable,
}

/// A trigger's selections into the same original owner arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TriggerMeaning {
    pub(crate) description_field: FlowFieldBinding,
    pub(crate) note_interpretation: NoteInterpretation,
    pub(crate) condition_field: FlowFieldBinding,
    pub(crate) condition: Option<SourceValueRef>,
    pub(crate) action_field: FlowFieldBinding,
    pub(crate) action: Option<SourceValueRef>,
}

/// A condition interpretation using original fields and flat child references.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum ConditionMeaning {
    TokenState {
        objective_token_field: FlowFieldBinding,
        objective: ObjectiveBinding,
        token_state_field: FlowFieldBinding,
        state: StateInterpretation,
    },
    AllTokensState {
        token_state_field: FlowFieldBinding,
        state: StateInterpretation,
    },
    AnyTokensState {
        token_state_field: FlowFieldBinding,
        state: StateInterpretation,
    },
    OnMissionStart,
    And {
        conditions_field: FlowFieldBinding,
        children: Vec<SourceValueRef>,
    },
    Or {
        conditions_field: FlowFieldBinding,
        children: Vec<SourceValueRef>,
    },
    Not {
        condition_field: FlowFieldBinding,
        child: Option<SourceValueRef>,
    },
    PropertyIsSet {
        property_field: FlowFieldBinding,
        property: PropertyBinding,
    },
    Unsupported,
}
