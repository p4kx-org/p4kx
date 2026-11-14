// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;

/// An owner-local preorder arena of original transformation occurrences.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DisplayTransformationSource<'source> {
    pub(crate) root: DisplayTransformationNodeRef,
    pub(crate) nodes: Vec<DisplayTransformationNodeSource<'source>>,
}

/// A structural link into this property's transformation arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DisplayTransformationNodeRef {
    #[schemars(range(min = u32::MIN, max = u32::MAX))]
    pub(crate) node_index: u32,
}

/// The independently sourced class and payload of one original owned occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DisplayTransformationNodeSource<'source> {
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) payload: SourcedValue<DisplayTransformationPayload>,
}

/// Supported authored transformation forms without arithmetic evaluation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum DisplayTransformationPayload {
    EmptyBase,
    ConvertFactorToNegatedPercentChange,
    ConvertFactorToPercentChange,
    ConvertValueToFactorOfBaseValue,
    Scale {
        scale: SourcedValue<ResourceNumber>,
    },
    Sequence {
        transformations: SourcedValue<Vec<SourcedValue<DisplayTransformationNodeRef>>>,
    },
}
