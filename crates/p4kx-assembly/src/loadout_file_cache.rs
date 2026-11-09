// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::LoadoutError;
use crate::loadout_file_trace::FileTrace;
use crate::resolver_observation::Observation;

pub(crate) struct CachedDocument<T> {
    value: T,
    trace: FileTrace,
}

impl<T> CachedDocument<T> {
    /// Retains the operation result and its original file observations together.
    pub(crate) fn read(run: impl FnOnce(&mut FileTrace) -> T) -> Self {
        let mut trace = FileTrace::default();
        let value = run(&mut trace);
        Self { value, trace }
    }

    /// Borrows the original interpreted result.
    pub(crate) const fn value(&self) -> &T {
        &self.value
    }

    /// Replays original observations before lending the cached result.
    pub(crate) fn reused(&self, observation: Option<&Observation>) -> Result<&T, LoadoutError> {
        self.trace.reused(observation)?;
        Ok(&self.value)
    }
}
