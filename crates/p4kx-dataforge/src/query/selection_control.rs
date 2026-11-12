// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicBool, Ordering};

use crate::ValueError;
use crate::query::source::QuerySource;

/// The shared, latched decision to continue selecting Records.
pub(super) struct Control<'a> {
    proceed: &'a (dyn Fn() -> bool + Sync),
    stopped: AtomicBool,
    observer: Option<&'a (dyn Fn(QuerySource) + Sync)>,
}

impl<'a> Control<'a> {
    /// Shares one cancellation decision across selection workers.
    pub(super) const fn new(
        proceed: &'a (dyn Fn() -> bool + Sync),
        observer: Option<&'a (dyn Fn(QuerySource) + Sync)>,
    ) -> Self {
        Self {
            proceed,
            stopped: AtomicBool::new(false),
            observer,
        }
    }

    pub(super) const fn observer(&self) -> Option<&'a (dyn Fn(QuerySource) + Sync)> {
        self.observer
    }

    /// Returns false after the callback has declined any work, even if it later returns true.
    pub(super) fn proceed(&self) -> bool {
        if self.stopped.load(Ordering::Relaxed) {
            return false;
        }
        if !(self.proceed)() {
            self.stopped.store(true, Ordering::Relaxed);
        }
        !self.stopped.load(Ordering::Relaxed)
    }
}

/// Why a demanded predicate could not be decided.
#[derive(Debug)]
pub(super) enum EvaluationError {
    Read {
        comparison: usize,
        source: ValueError,
    },
    Stopped,
}
