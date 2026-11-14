// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, FieldOrigin, PhysicalSourceRef};
use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// Structural navigation to one emitted physical Template owner.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PhysicalTemplateRef {
    pub(crate) input_ref: PhysicalSourceRef,
    pub(crate) record_index: u32,
}

/// A source field stored once at an arena root or in an original Struct.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum SourceFieldRef {
    Root {
        root_field_index: DecimalU64,
    },
    Field {
        parent: SourceValueRef,
        field_index: DecimalU64,
    },
}

/// Actual declaration presence or its original declaration-survey evidence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum DeclarationBinding {
    Present {
        field: SourceFieldRef,
    },
    Absent {
        #[serde(flatten)]
        origin: FieldOrigin,
    },
}

/// A named member of the actual Struct whose meaning contains this binding.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum FlowFieldBinding {
    Field {
        field_index: DecimalU64,
    },
    Absent {
        #[serde(flatten)]
        origin: FieldOrigin,
    },
}
