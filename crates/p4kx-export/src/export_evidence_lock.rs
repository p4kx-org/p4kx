// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Mutex, MutexGuard};

use crate::ExportAccountingError;

/// Refuses evidence whose owner unwound while mutating its protected state.
pub(crate) fn lock<T>(state: &Mutex<T>) -> Result<MutexGuard<'_, T>, ExportAccountingError> {
    state
        .lock()
        .map_err(|_| ExportAccountingError::PoisonedEvidence)
}

#[cfg(test)]
#[must_use]
pub(crate) fn poison<T: Send>(state: &Mutex<T>) -> bool {
    let unwound = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let Ok(_guard) = state.lock() else {
                    return;
                };
                panic!("synthetic evidence mutation interrupted");
            })
            .join()
            .is_err()
    });
    unwound && state.is_poisoned()
}
