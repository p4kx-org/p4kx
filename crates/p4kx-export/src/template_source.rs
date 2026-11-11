// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::{SourceField, SourceNode};
use crate::template_action_meaning::{ActionMeaning, RandomPhaseMeaning};
use crate::template_source_meaning::{ConditionMeaning, TriggerMeaning};

/// The single canonical raw inventory of one physical Template owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateSource<'source> {
    pub(crate) roots: Vec<TemplateSourceRoot<'source>>,
    pub(crate) values: Vec<TemplateSourceValue<'source>>,
}

/// One actual original Field root and its coordinates before selection into this arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateSourceRoot<'source> {
    pub(crate) source_position: OriginalFieldPosition,
    #[serde(flatten)]
    pub(crate) field: SourceField<'source>,
}

/// Original position controls, including an owning Struct that is not itself emitted.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct OriginalFieldPosition {
    pub(crate) parent_ordinal: Option<DecimalU64>,
    pub(crate) field_index: DecimalU64,
}
/// One exact original occurrence with separate nullable typed meanings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateSourceValue<'source> {
    pub(crate) source_ordinal: DecimalU64,
    #[serde(flatten)]
    pub(crate) original: SourceNode<'source>,
    pub(crate) condition: Option<ConditionMeaning>,
    pub(crate) action: Option<ActionMeaning>,
    pub(crate) random_phase: Option<RandomPhaseMeaning>,
    pub(crate) trigger: Option<TriggerMeaning>,
}
