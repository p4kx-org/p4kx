// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use crate::checkpoint::Checkpoint;
use crate::{Limits, MediaError};

#[derive(Default)]
pub(crate) struct InputBudget {
    pub(crate) sources: Cell<u64>,
    pub(crate) work: Cell<u64>,
    bytes: Cell<u64>,
}

impl InputBudget {
    pub(crate) const fn check_source(&self, limits: Limits) -> Result<(), MediaError> {
        check(self.sources.get(), 1, limits.sources, "sources")
    }

    pub(crate) fn admit_source(&self, limits: Limits) -> Result<(), MediaError> {
        admit(&self.sources, 1, limits.sources, "sources")
    }

    pub(crate) fn bytes(&self, count: u64, limits: Limits) -> Result<(), MediaError> {
        admit(&self.bytes, count, limits.source_bytes, "source_bytes")
    }

    #[must_use]
    pub(crate) const fn remaining_bytes(&self, limits: Limits) -> u64 {
        limits.source_bytes.saturating_sub(self.bytes.get())
    }

    pub(crate) const fn remaining_metadata_work(&self, limits: Limits) -> u64 {
        limits.metadata_work.saturating_sub(self.work.get())
    }

    pub(crate) fn metadata(
        &self,
        bytes: u64,
        work: u64,
        limits: Limits,
        checkpoint: Checkpoint<'_>,
    ) -> Result<(), MediaError> {
        checkpoint.check()?;
        check(self.bytes.get(), bytes, limits.source_bytes, "source_bytes")?;
        check(self.work.get(), work, limits.metadata_work, "metadata_work")?;
        self.bytes.set(self.bytes.get() + bytes);
        self.work.set(self.work.get() + work);
        checkpoint.check()
    }
}

fn admit(
    current: &Cell<u64>,
    added: u64,
    limit: u64,
    name: &'static str,
) -> Result<(), MediaError> {
    check(current.get(), added, limit, name)?;
    current.set(current.get() + added);
    Ok(())
}

const fn check(
    current: u64,
    added: u64,
    limit: u64,
    budget: &'static str,
) -> Result<(), MediaError> {
    if added > limit.saturating_sub(current) {
        return Err(MediaError::Limit { budget, limit });
    }
    Ok(())
}
