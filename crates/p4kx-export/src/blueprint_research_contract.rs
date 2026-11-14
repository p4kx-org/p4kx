// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// The declared research envelope and its supported interpretation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintResearch {
    pub(crate) source: Option<SourceValueRef>,
    pub(crate) envelope: SourcedValue<BlueprintResearchEnvelope>,
}

/// Both actual slots of the recognized research envelope.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintResearchEnvelope {
    pub(crate) research_costs: BlueprintResearchPointer,
    pub(crate) unlock_requirements: BlueprintResearchPointer,
}

/// The original pointer declaration, target base and support state.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct BlueprintResearchPointer {
    pub(crate) source: Option<SourceValueRef>,
    pub(crate) declared_base: SourcedValue<String>,
    pub(crate) value: SourcedValue<SourceValueRef>,
}
