// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A source float whose raw word survives non-finite values and signed zero.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Float32 {
    /// Finite JSON value; non-finite values are explicitly absent.
    value: Option<f32>,
    /// Original IEEE-754 word, independent of source byte order.
    raw_bits: String,
}

impl Float32 {
    /// Retains a source float's finite value and exact IEEE-754 word.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            value: value.is_finite().then_some(value),
            raw_bits: format!("{:08x}", value.to_bits()),
        }
    }
}

impl From<f32> for Float32 {
    fn from(value: f32) -> Self {
        Self::new(value)
    }
}

/// A source double whose raw word survives non-finite values and signed zero.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Float64 {
    /// Finite JSON value; non-finite values are explicitly absent.
    value: Option<f64>,
    /// Original IEEE-754 word, independent of source byte order.
    raw_bits: String,
}

impl Float64 {
    /// Retains a source double's finite value and exact IEEE-754 word.
    #[must_use]
    pub fn new(value: f64) -> Self {
        Self {
            value: value.is_finite().then_some(value),
            raw_bits: format!("{:016x}", value.to_bits()),
        }
    }
}

impl From<f64> for Float64 {
    fn from(value: f64) -> Self {
        Self::new(value)
    }
}

/// Origin of a projected value, independent of zero or an empty value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "origin", content = "value", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ValueOrigin<T> {
    /// Explicit source value.
    Explicit(T),
    /// Source contains no declaration.
    Missing,
    /// Declaration is present but its interpretation is unknown.
    Unknown,
    /// Value inherited through a separately preserved source reference.
    Inherited(T),
    /// Documented format default rather than an explicit source declaration.
    Default(T),
}
