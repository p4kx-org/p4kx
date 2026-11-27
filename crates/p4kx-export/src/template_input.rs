// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;
use crate::template_source_refs::DeclarationBinding;

/// A fixed Template projection input with its actual first-declaration selection.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateInput {
    pub(crate) declaration: DeclarationBinding,
    pub(crate) selected_struct: Option<SourceValueRef>,
}

/// Source sections used by the permanent Template context projection.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TemplateInputs {
    pub(crate) not_for_release: TemplateInput,
    pub(crate) work_in_progress: TemplateInput,
    pub(crate) display_info: TemplateInput,
    pub(crate) contract_class: TemplateInput,
    pub(crate) objective_tokens: TemplateInput,
    pub(crate) partial_reward_payout: TemplateInput,
    pub(crate) modifiers: TemplateInput,
    pub(crate) owner: TemplateInput,
    pub(crate) start_comms: TemplateInput,
    pub(crate) end_comms: TemplateInput,
}
