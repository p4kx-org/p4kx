// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::catalog_number_capture::{CapturedNumber, FloatToken};
use crate::catalog_row::CatalogRows;
use crate::catalog_target_index::TargetIndex;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::selection::Kind;

/// One thin original nonfinite token awaiting actual emitted raw coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NumberRequirement {
    pub(crate) record_index: u32,
    pub(crate) field: String,
    pub(crate) source_type: String,
    pub(crate) contract: ResourceNumberContract,
    pub(crate) token: FloatToken,
}

/// Actual numeric receipts attached to one emitted canonical family.
#[derive(Debug, Clone)]
pub(crate) struct EmittedNumber {
    pub(crate) kind: Kind,
    pub(crate) value: CapturedNumber,
}

/// Actual retained named-slot receipts indexed once by their original physical Record.
pub(crate) struct NumberCaptures<'source> {
    records: BTreeMap<u32, BTreeMap<&'source str, &'source CapturedNumber>>,
}

impl<'source> NumberCaptures<'source> {
    pub(crate) fn new(rows: Option<&'source CatalogRows>, targets: Option<&TargetIndex>) -> Self {
        let mut records: BTreeMap<u32, BTreeMap<&str, &CapturedNumber>> = BTreeMap::new();
        if let (Some(rows), Some(targets)) = (rows, targets) {
            for number in &rows.numbers {
                let actual = &number.value;
                if targets
                    .emitted
                    .contains(&(number.kind, actual.record_index))
                {
                    records
                        .entry(actual.record_index)
                        .or_default()
                        .entry(actual.field.as_str())
                        .or_insert(actual);
                }
            }
        }
        Self { records }
    }

    pub(crate) fn for_record(&self, record_index: u32) -> Coverage<'_, 'source> {
        Coverage {
            record_index,
            available: self.records.get(&record_index),
            consumed: BTreeSet::new(),
        }
    }
}

/// Fresh consumption state for one original selected or dependency inspection.
pub(crate) struct Coverage<'index, 'source> {
    record_index: u32,
    available: Option<&'index BTreeMap<&'source str, &'source CapturedNumber>>,
    consumed: BTreeSet<&'source str>,
}

impl Coverage<'_, '_> {
    pub(crate) fn take(&mut self, required: &NumberRequirement) -> bool {
        if required.record_index != self.record_index {
            return false;
        }
        let Some((field, actual)) = self
            .available
            .and_then(|values| values.get_key_value(required.field.as_str()))
        else {
            return false;
        };
        actual.token == required.token
            && actual.source_type == required.source_type
            && actual.contract == required.contract
            && self.consumed.insert(*field)
    }
}
