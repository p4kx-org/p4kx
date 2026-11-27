// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_comparison::{Filter, OwnedComparison};
use p4kx_extraction::CancelToken;

use crate::model::describe;

/// Where saving the JSON report stands.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReportStatus {
    /// The report is being written to the path.
    Saving(PathBuf),
    /// The report was written to the path.
    Saved(PathBuf),
    /// The report could not be written to the path, for the reason given.
    Failed(PathBuf, String),
}

/// Saving the report on a background thread, or its outcome.
#[derive(Debug)]
pub(crate) struct ReportJob {
    status: ReportStatus,
    cancel: CancelToken,
    result: Option<Receiver<Result<(), String>>>,
}

impl Drop for ReportJob {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl ReportJob {
    /// Returns where saving stands.
    pub(crate) const fn status(&self) -> &ReportStatus {
        &self.status
    }

    /// Takes the outcome once saving ended; returns whether it ended.
    pub(crate) fn poll(&mut self) -> bool {
        let Some(result) = &self.result else {
            return false;
        };
        let outcome = match result.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err(String::new()),
        };
        self.result = None;
        let path = path_of(&self.status).to_path_buf();
        self.status = match outcome {
            Ok(()) => ReportStatus::Saved(path),
            Err(reason) => ReportStatus::Failed(path, reason),
        };
        true
    }
}

fn path_of(status: &ReportStatus) -> &Path {
    match status {
        ReportStatus::Saving(path) | ReportStatus::Saved(path) | ReportStatus::Failed(path, _) => {
            path.as_path()
        }
    }
}

/// Compares again and writes the report of `comparison`, narrowed to `filter` when given, to
/// `path` on a background thread, calling `wake` from it once it ended.
pub(crate) fn start(
    comparison: &Arc<OwnedComparison>,
    filter: Option<Filter>,
    path: PathBuf,
    wake: impl Fn() + Send + 'static,
) -> ReportJob {
    let (sender, result) = mpsc::channel();
    let cancel = CancelToken::new();
    let thread_cancel = cancel.clone();
    let thread_comparison = Arc::clone(comparison);
    let thread_path = path.clone();
    let spawned = thread::Builder::new()
        .name("comparison-report".to_owned())
        .spawn(move || {
            let saved = thread_comparison
                .recompare(filter.as_ref(), Some(&thread_cancel), |comparison| {
                    comparison.save_report_with_cancel(&thread_path, Some(&thread_cancel))
                })
                .map_err(|error| describe(&error))
                .and_then(|written| written.map_err(|error| describe(&error)));
            if sender.send(saved).is_ok() {
                wake();
            }
        });
    match spawned {
        Ok(_handle) => ReportJob {
            status: ReportStatus::Saving(path),
            cancel,
            result: Some(result),
        },
        Err(error) => ReportJob {
            status: ReportStatus::Failed(path, describe(&error)),
            cancel,
            result: None,
        },
    }
}
