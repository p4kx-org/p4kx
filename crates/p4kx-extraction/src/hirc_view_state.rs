// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::{StateObject, StateProperty};
use schemars::JsonSchema;
use serde::Serialize;

use crate::DecimalU64;

/// Original metadata of a standalone Wwise State.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct StateDetail {
    /// Authored property count.
    pub property_count: u16,
    /// Original range of the count word.
    pub count_range: StateRange,
    /// Original ordered property occurrences.
    pub properties: Vec<StatePropertyView>,
    /// Original extent of the property bundle.
    pub consumed_range: StateRange,
    /// Original extent after the property bundle.
    pub trailing_range: StateRange,
}

/// One original standalone State property.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct StatePropertyView {
    /// The raw RTPC parameter ID.
    pub id: u16,
    /// Original byte range of the ID.
    pub id_range: StateRange,
    /// Exact original binary32 bits.
    pub value_bits: u32,
    /// Finite numeric value, or null for an infinite or NaN word.
    pub value: Option<f32>,
    /// Original byte range of the binary32 word.
    pub value_range: StateRange,
}

/// A half-open byte range in the decoded bank.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct StateRange {
    /// Inclusive bank-relative start.
    pub start: DecimalU64,
    /// Exclusive bank-relative end.
    pub end_exclusive: DecimalU64,
}

pub(crate) fn detail(state: &StateObject) -> StateDetail {
    let count = state.properties.count();
    StateDetail {
        property_count: count.value,
        count_range: StateRange::new(&count.range),
        properties: state
            .properties
            .entries()
            .map(|entry| property_view(&entry))
            .collect(),
        consumed_range: StateRange::new(&state.properties.consumed_range()),
        trailing_range: StateRange::new(&state.trailing_range),
    }
}

fn property_view(property: &StateProperty) -> StatePropertyView {
    let value = property.value();
    StatePropertyView {
        id: property.id.value,
        id_range: StateRange::new(&property.id.range),
        value_bits: property.value_bits.value,
        value: value.is_finite().then_some(value),
        value_range: StateRange::new(&property.value_bits.range),
    }
}

impl StateRange {
    pub(super) const fn new(range: &Range<u64>) -> Self {
        Self {
            start: DecimalU64::new(range.start),
            end_exclusive: DecimalU64::new(range.end),
        }
    }
}
