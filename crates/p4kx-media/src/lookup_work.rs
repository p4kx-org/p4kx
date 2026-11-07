// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use crate::checkpoint::Checkpoint;
use crate::{Limits, MediaError};

pub(crate) struct LookupWork<'a> {
    pub(crate) checkpoint: Checkpoint<'a>,
    pub(crate) limits: Limits,
    pub(crate) current: &'a Cell<u64>,
    pub(crate) source_allowance: u64,
}

impl LookupWork<'_> {
    pub(crate) fn charge(&self, count: u64) -> Result<(), MediaError> {
        self.checkpoint.check()?;
        let current = self.current.get();
        if count > self.limits.metadata_work.saturating_sub(current) {
            return Err(MediaError::Limit {
                budget: "metadata_work",
                limit: self.limits.metadata_work,
            });
        }
        self.current.set(current + count);
        self.checkpoint.check()
    }
}
