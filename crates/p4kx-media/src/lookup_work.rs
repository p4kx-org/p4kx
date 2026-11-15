// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::checkpoint::Checkpoint;
use crate::input_budget::InputBudget;
use crate::{Limits, MediaError};

pub(crate) struct LookupWork<'a> {
    pub(crate) checkpoint: Checkpoint<'a>,
    pub(crate) limits: Limits,
    pub(crate) budget: &'a InputBudget,
    pub(crate) source_allowance: u64,
}

impl LookupWork<'_> {
    pub(crate) fn charge(&self, count: u64) -> Result<(), MediaError> {
        self.budget.metadata(0, count, self.limits, self.checkpoint)
    }
}
