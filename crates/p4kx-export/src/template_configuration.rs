// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::mission_flow_owner::MissionFlowOwnerRef;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_source_refs::{DeclarationBinding, SourceFieldRef};

/// One actual root Flow declaration or its genuinely absent declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionFlowConfiguration {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) declaration_index: Option<DecimalU64>,
    pub(crate) declaration: DeclarationBinding,
    pub(crate) selection: FlowSelection,
    pub(crate) triggers: Vec<SourceValueRef>,
    pub(crate) referenced_fields: Vec<SourceFieldRef>,
}

/// The preserved first-declaration/first-Struct source selection recipe.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum FlowSelection {
    Absent,
    ShadowedDeclaration,
    NoStruct,
    Selected { value: SourceValueRef },
}

/// A typed link to one Flow declaration of its physical mission owner.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionFlowRef {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) declaration_index: Option<DecimalU64>,
}

/// One original mission or objective property declaration without a second raw arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionPropertyConfiguration {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) owner_objective: Option<SourceValueRef>,
    pub(crate) declaration_index: Option<DecimalU64>,
    pub(crate) declaration: DeclarationBinding,
    pub(crate) selection: PropertySelection,
    pub(crate) properties: Vec<SourceValueRef>,
}

/// The original declaration and objective selection independently of retained source facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum PropertySelection {
    Absent,
    ShadowedDeclaration,
    ShadowedObjective,
    NoStruct,
    Selected,
}

/// A typed link to one original property declaration of its physical owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionPropertyRef {
    pub(crate) owner: MissionFlowOwnerRef,
    pub(crate) owner_objective: Option<SourceValueRef>,
    pub(crate) declaration_index: Option<DecimalU64>,
}
