// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::schema_fields;

/// A quality distribution with exact source integer bounds and floating-point parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct QualityCurve<'source> {
    /// The concrete source distribution struct.
    pub kind: &'source str,
    /// The exact source minimum quality, absent only for a missing or null field.
    #[schemars(range(min = i32::MIN, max = i32::MAX))]
    pub min: Option<i32>,
    /// The exact source maximum quality, absent only for a missing or null field.
    #[schemars(range(min = i32::MIN, max = i32::MAX))]
    pub max: Option<i32>,
    /// The finite source mean, represented as a 32-bit float.
    pub mean: Option<f32>,
    /// The finite source standard deviation, represented as a 32-bit float.
    pub stddev: Option<f32>,
}
