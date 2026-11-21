// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_comparison::{Filter, OwnedComparison};
use rayon::prelude::*;

use super::summary::Summary;

/// The changes a Filter matched, indexed by change id, and their counts.
#[derive(Debug)]
pub(crate) struct Matched {
    pub(crate) mask: Arc<[bool]>,
    pub(crate) summary: Summary,
}

/// A filter that was started.
pub(crate) enum Started {
    Running(FilterJob),
    Finished(Matched),
}

/// What became of a running filter.
pub(crate) enum Polled {
    Running,
    Finished(Matched),
    Lost,
}

/// A filter running on a background thread; dropping it cancels the thread.
#[derive(Debug)]
pub(crate) struct FilterJob {
    cancelled: Arc<AtomicBool>,
    result: Receiver<Matched>,
}

impl FilterJob {
    /// Takes the result once the filter finished.
    pub(crate) fn poll(&self) -> Polled {
        match self.result.try_recv() {
            Ok(matched) => Polled::Finished(matched),
            Err(TryRecvError::Empty) => Polled::Running,
            Err(TryRecvError::Disconnected) => Polled::Lost,
        }
    }
}

impl Drop for FilterJob {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

/// Returns the match of a Filter that selects no change of `comparison`.
pub(crate) fn none(comparison: &OwnedComparison) -> Matched {
    let mask = vec![false; comparison.len()];
    Matched {
        summary: Summary::of(&comparison.comparison_of(&mask)),
        mask: mask.into(),
    }
}

/// Matches `filter` against every change of `comparison` on a background thread, calling `wake`
/// from it once the result is ready; matches on the calling thread when no thread can start.
pub(crate) fn start(
    comparison: &Arc<OwnedComparison>,
    filter: &Filter,
    wake: impl Fn() + Send + 'static,
) -> Started {
    let cancelled = Arc::new(AtomicBool::new(false));
    let (sender, result) = mpsc::channel();
    let thread_comparison = Arc::clone(comparison);
    let thread_filter = filter.clone();
    let thread_cancelled = Arc::clone(&cancelled);
    let spawned = thread::Builder::new()
        .name("comparison-filter".to_owned())
        .spawn(move || {
            if let Some(matched) = matching(&thread_comparison, &thread_filter, &thread_cancelled)
                && sender.send(matched).is_ok()
            {
                wake();
            }
        });
    match spawned {
        Ok(_handle) => Started::Running(FilterJob { cancelled, result }),
        Err(_ignored) => Started::Finished(
            matching(comparison, filter, &AtomicBool::new(false)).unwrap_or_else(|| Matched {
                mask: Arc::from([]),
                summary: Summary::default(),
            }),
        ),
    }
}

fn matching(
    comparison: &OwnedComparison,
    filter: &Filter,
    cancelled: &AtomicBool,
) -> Option<Matched> {
    let mask: Vec<bool> = (0..comparison.len())
        .into_par_iter()
        .map(|id| {
            !cancelled.load(Ordering::Relaxed)
                && comparison
                    .entry_change(id)
                    .is_some_and(|change| filter.matches(change.path()))
        })
        .collect();
    if cancelled.load(Ordering::Relaxed) {
        return None;
    }
    let incomparables = comparison
        .incomparable_entries()
        .take_while(|_| !cancelled.load(Ordering::Relaxed))
        .filter(|entry| filter.matches(entry.path()))
        .count();
    let summary = Summary {
        incomparables,
        ..Summary::of(&comparison.comparison_of(&mask))
    };
    (!cancelled.load(Ordering::Relaxed)).then(|| Matched {
        mask: mask.into(),
        summary,
    })
}
