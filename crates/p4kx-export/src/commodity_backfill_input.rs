// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::Guid;

const MAX_INPUTS: usize = 4096;
const MAX_INPUT_BYTES: usize = 1024 * 1024;

/// The observed resource and optional quantization used by one eligibility input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CommodityInputTarget {
    pub(crate) resource: Guid,
    pub(crate) quantization: Option<Guid>,
}
/// An actual decision input observed inside the existing crafting projection.
#[derive(Debug, Clone)]
pub(crate) struct CommodityBackfillInput {
    pub(crate) resource: Guid,
    pub(crate) quantization: Option<Guid>,
    pub(crate) record_index: u32,
    pub(crate) field: String,
    pub(crate) selectors: Vec<String>,
}

/// A bounded origin capture at the actual reader's decision points.
#[must_use]
pub(crate) struct CommodityInputCapture {
    record_index: u32,
    inputs: Vec<CommodityBackfillInput>,
    bytes: usize,
    limited: bool,
    limited_targets: BTreeSet<CommodityInputTarget>,
    target_inventory_limited: bool,
}

/// Capturing required decision inputs exceeded its per-Record metadata budget.
#[derive(Debug, Clone)]
pub(crate) struct CommodityInputLimit {
    pub(crate) record_index: u32,
    pub(crate) target: Option<CommodityInputTarget>,
}

/// Retained decision metadata and any separate capture-limit observation.
#[must_use]
pub(crate) struct CommodityCapturedInputs {
    pub(crate) inputs: Vec<CommodityBackfillInput>,
    pub(crate) limits: Vec<CommodityInputLimit>,
}

impl CommodityInputCapture {
    pub(crate) const fn new(record_index: u32) -> Self {
        Self {
            record_index,
            inputs: Vec::new(),
            bytes: 0,
            limited: false,
            limited_targets: BTreeSet::new(),
            target_inventory_limited: false,
        }
    }

    pub(crate) fn observe(
        &mut self,
        resource: Guid,
        quantization: Option<Guid>,
        field: &str,
        selectors: &[String],
    ) {
        let bytes = selectors
            .len()
            .checked_mul(size_of::<String>())
            .and_then(|storage| storage.checked_add(size_of::<CommodityBackfillInput>()))
            .and_then(|storage| storage.checked_add(field.len()))
            .and_then(|initial| {
                selectors
                    .iter()
                    .try_fold(initial, |total, field| total.checked_add(field.len()))
            });
        let Some(bytes) = bytes else {
            self.limit(resource, quantization);
            return;
        };
        if self.limited
            || self.inputs.len() == MAX_INPUTS
            || bytes > MAX_INPUT_BYTES.saturating_sub(self.bytes)
        {
            self.limit(resource, quantization);
            return;
        }
        self.bytes += bytes;
        self.inputs.push(CommodityBackfillInput {
            resource,
            quantization,
            record_index: self.record_index,
            field: field.to_owned(),
            selectors: selectors.to_vec(),
        });
    }

    fn limit(&mut self, resource: Guid, quantization: Option<Guid>) {
        self.limited = true;
        let target = CommodityInputTarget {
            resource,
            quantization,
        };
        if self.limited_targets.contains(&target) {
            return;
        }
        if self.limited_targets.len() == MAX_INPUTS {
            self.target_inventory_limited = true;
        } else {
            self.limited_targets.insert(target);
        }
    }

    pub(crate) fn finish(self) -> CommodityCapturedInputs {
        let mut limits: Vec<_> = self
            .limited_targets
            .into_iter()
            .map(|target| CommodityInputLimit {
                record_index: self.record_index,
                target: Some(target),
            })
            .collect();
        if self.target_inventory_limited {
            limits.push(CommodityInputLimit {
                record_index: self.record_index,
                target: None,
            });
        }
        CommodityCapturedInputs {
            inputs: self.inputs,
            limits,
        }
    }
}
