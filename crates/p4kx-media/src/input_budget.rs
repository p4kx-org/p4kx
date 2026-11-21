// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Mutex, MutexGuard};

use crate::checkpoint::Checkpoint;
use crate::{Limits, MediaError};

#[derive(Default)]
pub(crate) struct InputBudget {
    counts: Mutex<Counts>,
}

#[derive(Default)]
struct Counts {
    sources: u64,
    work: u64,
    bytes: u64,
}

impl InputBudget {
    fn counts(&self) -> Result<MutexGuard<'_, Counts>, MediaError> {
        self.counts
            .lock()
            .map_err(|_| MediaError::Contract("input budget mutex is poisoned".into()))
    }

    pub(crate) fn source_count(&self) -> Result<u64, MediaError> {
        Ok(self.counts()?.sources)
    }

    #[cfg(test)]
    pub(crate) fn work_count(&self) -> Result<u64, MediaError> {
        Ok(self.counts()?.work)
    }

    pub(crate) fn check_source(&self, limits: Limits) -> Result<(), MediaError> {
        check(self.source_count()?, 1, limits.sources, "sources")
    }

    pub(crate) fn admit_source(&self, limits: Limits) -> Result<(), MediaError> {
        admit(&mut self.counts()?.sources, 1, limits.sources, "sources")
    }

    pub(crate) fn bytes(&self, count: u64, limits: Limits) -> Result<(), MediaError> {
        admit(
            &mut self.counts()?.bytes,
            count,
            limits.source_bytes,
            "source_bytes",
        )
    }

    pub(crate) fn remaining_bytes(&self, limits: Limits) -> Result<u64, MediaError> {
        Ok(limits.source_bytes.saturating_sub(self.counts()?.bytes))
    }

    pub(crate) fn remaining_metadata_work(&self, limits: Limits) -> Result<u64, MediaError> {
        Ok(limits.metadata_work.saturating_sub(self.counts()?.work))
    }

    pub(crate) fn metadata(
        &self,
        bytes: u64,
        work: u64,
        limits: Limits,
        checkpoint: Checkpoint<'_>,
    ) -> Result<(), MediaError> {
        checkpoint.check()?;
        {
            let mut counts = self.counts()?;
            check(counts.bytes, bytes, limits.source_bytes, "source_bytes")?;
            check(counts.work, work, limits.metadata_work, "metadata_work")?;
            counts.bytes += bytes;
            counts.work += work;
        }
        checkpoint.check()
    }
}

fn admit(current: &mut u64, added: u64, limit: u64, name: &'static str) -> Result<(), MediaError> {
    check(*current, added, limit, name)?;
    *current += added;
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
