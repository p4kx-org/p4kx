// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use super::hirc_path_source::PathSourceMode;

/// Controls traversal and source evidence for one audio trigger.
#[derive(Clone, Copy)]
pub struct ResolveOptions<'a> {
    pub(super) max_visits: Option<usize>,
    pub(super) cancel: Option<&'a CancelToken>,
    pub(super) path_sources: PathSourceMode,
}

impl Default for ResolveOptions<'_> {
    fn default() -> Self {
        Self {
            max_visits: None,
            cancel: None,
            path_sources: PathSourceMode::Omit,
        }
    }
}

impl<'a> ResolveOptions<'a> {
    /// Limits attempted HIRC objects and music track sources across the whole trigger.
    ///
    /// The count includes failed object lookups and excludes construction of the audio index.
    #[must_use]
    pub const fn with_max_visits(mut self, limit: usize) -> Self {
        self.max_visits = Some(limit);
        self
    }

    /// Stops resolution when the caller cancels the token.
    #[must_use]
    pub const fn with_cancel(mut self, cancel: &'a CancelToken) -> Self {
        self.cancel = Some(cancel);
        self
    }

    /// Retains the original HIRC byte spans of the visited path steps.
    #[must_use]
    pub const fn with_path_sources(mut self) -> Self {
        self.path_sources = PathSourceMode::Retain;
        self
    }

    pub(super) fn cancelled(self) -> bool {
        self.cancel.is_some_and(CancelToken::is_cancelled)
    }
}

pub(super) enum WorkStop {
    Budget { limit: usize },
    Cancelled,
}

struct VisitBudget {
    limit: usize,
    remaining: usize,
    exhausted: bool,
}

pub(super) struct ResolveWork<'a> {
    cancel: Option<&'a CancelToken>,
    budget: Option<VisitBudget>,
}

impl<'a> ResolveWork<'a> {
    pub(super) fn new(options: ResolveOptions<'a>) -> Self {
        Self {
            cancel: options.cancel,
            budget: options.max_visits.map(|limit| VisitBudget {
                limit,
                remaining: limit,
                exhausted: false,
            }),
        }
    }

    pub(super) fn cancelled(&self) -> bool {
        self.cancel.is_some_and(CancelToken::is_cancelled)
    }

    pub(super) fn exhausted_budget(&self) -> bool {
        self.budget.as_ref().is_some_and(|budget| budget.exhausted)
    }

    pub(super) fn admit(&mut self) -> Result<(), WorkStop> {
        if self.cancelled() {
            return Err(WorkStop::Cancelled);
        }
        if let Some(budget) = &mut self.budget
            && budget.remaining == 0
        {
            budget.exhausted = true;
            return Err(WorkStop::Budget {
                limit: budget.limit,
            });
        }
        Ok(())
    }

    pub(super) fn spend(&mut self) -> Result<(), WorkStop> {
        self.admit()?;
        if let Some(budget) = &mut self.budget {
            budget.remaining -= 1;
        }
        Ok(())
    }
}
