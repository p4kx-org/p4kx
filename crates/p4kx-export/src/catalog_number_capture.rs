// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::resource_raw_policy::ResourceNumberContract;
use p4kx_extraction::Value;

/// Exact scalar storage of one nonfinite source number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum FloatToken {
    Single(u32),
    Double(u64),
}

impl FloatToken {
    pub(crate) const fn read(value: &Value<'_>) -> Option<Self> {
        match value {
            Value::Single(value) if !value.is_finite() => Some(Self::Single(value.to_bits())),
            Value::Double(value) if !value.is_finite() => Some(Self::Double(value.to_bits())),
            _ => None,
        }
    }
}

/// A lossless numeric token retained by the actual raw-field reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedNumber {
    pub(crate) record_index: u32,
    pub(crate) field: String,
    pub(crate) token: FloatToken,
    pub(crate) source_type: String,
    pub(crate) contract: ResourceNumberContract,
}

/// Local evidence committed only with its successfully returned canonical definition.
#[derive(Default)]
pub(crate) struct NumberCapture {
    values: RefCell<BTreeMap<(u32, String), CapturedScalar>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapturedScalar {
    source_type: String,
    contract: ResourceNumberContract,
    token: FloatToken,
}

impl NumberCapture {
    pub(crate) fn retain(
        &self,
        record_index: u32,
        field: impl FnOnce() -> String,
        source_type: &str,
        contract: ResourceNumberContract,
        value: &Value<'_>,
    ) {
        if let Some(token) = FloatToken::read(value) {
            self.values
                .borrow_mut()
                .entry((record_index, field()))
                .or_insert_with(|| CapturedScalar {
                    source_type: source_type.to_owned(),
                    contract,
                    token,
                });
        }
    }

    pub(crate) fn finish(self) -> Vec<CapturedNumber> {
        self.values
            .into_inner()
            .into_iter()
            .map(|((record_index, field), scalar)| CapturedNumber {
                record_index,
                field,
                source_type: scalar.source_type,
                contract: scalar.contract,
                token: scalar.token,
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "catalog_number_capture_tests.rs"]
mod tests;
