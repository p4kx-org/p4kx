// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;

/// An owner-local preorder arena of original transformation occurrences.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct DisplayTransformationSource<'source> {
    /// The arena index of the outermost transformation.
    pub root: DisplayTransformationNodeRef,
    /// The source occurrences in preorder.
    pub nodes: Vec<DisplayTransformationNodeSource<'source>>,
}

/// A structural link into this property's transformation arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct DisplayTransformationNodeRef {
    /// The zero-based index into the arena.
    #[schemars(range(min = u32::MIN, max = u32::MAX))]
    pub node_index: u32,
}

/// The independently sourced class and payload of one original owned occurrence.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct DisplayTransformationNodeSource<'source> {
    /// The concrete source class.
    pub source_type: SourcedValue<Cow<'source, str>>,
    /// Fields recognized for this class.
    pub payload: SourcedValue<DisplayTransformationPayload>,
}

/// Supported authored transformation forms without arithmetic evaluation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum DisplayTransformationPayload {
    /// The base transformation has no fields.
    EmptyBase,
    /// Converts a factor to the negated percentage change.
    ConvertFactorToNegatedPercentChange,
    /// Converts a factor to a percentage change.
    ConvertFactorToPercentChange,
    /// Converts a value to a factor relative to the base value.
    ConvertValueToFactorOfBaseValue,
    /// Provides an authored scale.
    Scale {
        /// The stored scale with its original numeric representation.
        scale: SourcedValue<ResourceNumber>,
    },
    /// Provides a sequence of transformation links.
    Sequence {
        /// The ordered links to the sequence's transformation occurrences.
        transformations: SourcedValue<Vec<SourcedValue<DisplayTransformationNodeRef>>>,
    },
}
