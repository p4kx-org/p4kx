// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::SystemTime;

use p4kx_export::{
    CancelToken, ConflictPolicy, RecordExportOptions, RecordForm, write_planned_record_export,
};
use p4kx_extraction::{
    ArchiveWriteProtection, Filter, Record, TagResolver, create_default_target_protected,
};
use rayon::prelude::*;

use super::RecordExportSummary;
use super::progress::{RecordExportProgress, RecordExportStage, Shared};
use crate::extraction::target_naming;
use crate::forge_cache::{ForgeSource, ReadFailure};
use crate::model::describe;
use crate::{EntryIndex, TargetKind};

/// The Records a Record export writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Selection {
    /// The Records at these positions in definition order.
    Records(Arc<[u32]>),
    /// The Records whose record path matches this Filter.
    Filter(Filter),
}

/// What a Record export writes and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) selection: Selection,
    pub(crate) form: RecordForm,
    pub(crate) tags: bool,
    pub(crate) kind: TargetKind,
    pub(crate) folder: PathBuf,
    pub(crate) policy: ConflictPolicy,
}

/// Where a Record export stands.
#[derive(Debug, Clone, Copy)]
pub enum RecordExportStatus<'a> {
    /// The export runs; the progress so far.
    Running(RecordExportProgress),
    /// Every Record ended, or the export was cancelled.
    Finished(&'a RecordExportSummary),
    /// DataForge or the target could not be prepared, for the reason given, or the export
    /// stopped without a result.
    Failed(Option<&'a str>),
}

type RunResult = Result<RecordExportSummary, String>;

#[derive(Debug)]
enum State {
    Running(Receiver<RunResult>),
    Finished(RecordExportSummary),
    Failed(Option<String>),
}

/// A Record export running on a background thread, or its outcome.
#[derive(Debug)]
pub struct RecordExport {
    id: u64,
    folder: PathBuf,
    form: RecordForm,
    cancel: CancelToken,
    shared: Arc<Shared>,
    state: State,
}

impl RecordExport {
    /// Returns the number that tells this export apart from every other one of the process.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Returns the chosen folder: the parent of a new folder, or the existing folder.
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// Returns where the export stands.
    #[must_use]
    pub fn status(&self) -> RecordExportStatus<'_> {
        match &self.state {
            State::Running(_) => RecordExportStatus::Running(self.shared.progress()),
            State::Finished(summary) => RecordExportStatus::Finished(summary),
            State::Failed(reason) => RecordExportStatus::Failed(reason.as_deref()),
        }
    }

    /// Returns whether the export still runs.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.state, State::Running(_))
    }

    /// Returns whether the export was asked to stop.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Asks the export to stop; Records not yet written are reported as cancelled.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Takes the outcome once the export ended; returns whether it ended.
    pub(crate) fn poll(&mut self) -> bool {
        let State::Running(result) = &self.state else {
            return false;
        };
        self.state = match result.try_recv() {
            Ok(outcome) => ended(Some(outcome)),
            Err(TryRecvError::Empty) => return self.cancel.is_cancelled() && self.give_up(),
            Err(TryRecvError::Disconnected) => ended(None),
        };
        true
    }

    /// Cancels the export and waits until it ended, partial files removed.
    pub(crate) fn stop(&mut self) {
        self.cancel.cancel();
        if self.give_up() {
            return;
        }
        let State::Running(result) = &self.state else {
            return;
        };
        self.state = ended(result.recv().ok());
    }

    /// Ends a cancelled export at once while it still reads DataForge, which writes nothing; the
    /// thread finishes reading and keeps DataForge for the next export. Returns whether it ended.
    fn give_up(&mut self) -> bool {
        if !matches!(self.state, State::Running(_)) || !self.shared.give_up() {
            return false;
        }
        self.state = State::Finished(RecordExportSummary::nothing_written(self.form, 0, true));
        true
    }
}

fn ended(outcome: Option<RunResult>) -> State {
    match outcome {
        Some(Ok(summary)) => State::Finished(summary),
        Some(Err(reason)) => State::Failed(Some(reason)),
        None => State::Failed(None),
    }
}

/// Starts writing the Records of `request` from the DataForge of `index` on a background thread,
/// reading DataForge into the cache of `index` unless it holds it already, and calls `wake` from
/// that thread once the export ended.
pub(crate) fn start(
    index: &Arc<EntryIndex>,
    request: Request,
    wake: impl Fn() + Send + 'static,
    #[cfg(feature = "test-hooks")] pause: crate::hold::Pause,
) -> RecordExport {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let cancel = CancelToken::new();
    let shared = Arc::new(Shared::default());
    let folder = request.folder.clone();
    let form = request.form;
    let (sender, result) = mpsc::channel();
    let thread_index = Arc::clone(index);
    let thread_cancel = cancel.clone();
    let thread_shared = Arc::clone(&shared);
    let spawned = thread::Builder::new()
        .name("record-export".to_owned())
        .spawn(move || {
            let job = Job {
                index: &thread_index,
                request: &request,
                cancel: &thread_cancel,
                shared: &thread_shared,
                #[cfg(feature = "test-hooks")]
                pause,
            };
            if sender.send(job.run()).is_ok() {
                wake();
            }
        });
    let state = match spawned {
        Ok(_handle) => State::Running(result),
        Err(error) => State::Failed(Some(describe(&error))),
    };
    RecordExport {
        id,
        folder,
        form,
        cancel,
        shared,
        state,
    }
}

struct Job<'a> {
    index: &'a EntryIndex,
    request: &'a Request,
    cancel: &'a CancelToken,
    shared: &'a Shared,
    #[cfg(feature = "test-hooks")]
    pause: crate::hold::Pause,
}

impl Job<'_> {
    fn run(&self) -> RunResult {
        let source = self.source()?;
        if !self.shared.leave_reading() {
            return Err(String::new());
        }
        let forge = source.entry.open().map_err(|error| describe(&error))?;
        self.shared.enter(RecordExportStage::Selecting);
        let all: Vec<Record<'_>> = forge.records().collect();
        let records = select(&all, &self.request.selection);
        let stop = || self.cancel.is_cancelled();
        let tags = (self.request.tags && !records.is_empty())
            .then(|| source.tags(&stop))
            .transpose()
            .map_err(ReadFailure::into_reason)?;
        let stopped = matches!(tags, Some(None));
        if records.is_empty() || stopped || self.cancel.is_cancelled() {
            return Ok(RecordExportSummary::nothing_written(
                self.request.form,
                records.len(),
                stopped || self.cancel.is_cancelled(),
            ));
        }
        let tags = tags.flatten();
        let cancelled =
            || RecordExportSummary::nothing_written(self.request.form, records.len(), true);
        let protection = match ArchiveWriteProtection::new(self.index.path(), self.cancel.flag()) {
            Ok(protection) => protection,
            Err(error) if error.is_cancelled() => return Ok(cancelled()),
            Err(error) => return Err(describe(&error)),
        };
        let target = match prepare_target(self.index, self.request, &protection, self.cancel) {
            Ok(target) => target,
            Err(error) if crate::installation::guard_cancelled(&error) => return Ok(cancelled()),
            Err(error) => return Err(describe(&error)),
        };
        self.shared.total.store(records.len(), Ordering::Relaxed);
        self.shared.enter(RecordExportStage::Writing);
        #[cfg(feature = "test-hooks")]
        self.pause.wait(|| self.cancel.is_cancelled());
        let progress = || {
            self.shared.done.fetch_add(1, Ordering::Relaxed);
        };
        let options = RecordExportOptions {
            policy: self.request.policy,
            cancel: Some(self.cancel),
            progress: Some(&progress),
            texts: None,
            tags: tags.as_deref().map(|tags| tags as &dyn TagResolver),
            protection: Some(&protection),
        };
        let summary = write_planned_record_export(
            &forge,
            &source.plans,
            &records,
            self.request.form,
            &target,
            &options,
        );
        Ok(RecordExportSummary::new(target, records.len(), &summary))
    }

    /// Returns DataForge from the cache of the Archive, reading it first when the cache is empty.
    fn source(&self) -> Result<Arc<ForgeSource>, String> {
        self.index
            .watch()
            .check()
            .map_err(|error| describe(&error))?;
        self.index
            .forge()
            .read(self.index, || {})
            .map_err(ReadFailure::into_reason)
    }
}

/// Returns the Records of `all` that `selection` names, each once.
fn select<'f>(all: &[Record<'f>], selection: &Selection) -> Vec<Record<'f>> {
    match selection {
        Selection::Records(positions) => {
            let mut positions = positions.to_vec();
            positions.sort_unstable();
            positions.dedup();
            positions
                .iter()
                .filter_map(|&position| all.get(position as usize).copied())
                .collect()
        }
        Selection::Filter(filter) => all
            .par_iter()
            .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
            .copied()
            .collect(),
    }
}

fn prepare_target(
    index: &EntryIndex,
    request: &Request,
    protection: &ArchiveWriteProtection,
    cancel: &CancelToken,
) -> io::Result<PathBuf> {
    protection
        .ensure_writable(&request.folder, cancel.flag())
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
    match request.kind {
        TargetKind::ExistingFolder => {
            fs::create_dir_all(&request.folder)?;
            protection
                .ensure_writable(&request.folder, cancel.flag())
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            Ok(request.folder.clone())
        }
        TargetKind::NewFolder => {
            let (channel, build) = target_naming(index.path(), index.build_of_archive());
            create_default_target_protected(
                &request.folder,
                channel.as_deref(),
                build,
                SystemTime::now(),
                protection,
                cancel,
            )
        }
    }
}

#[cfg(test)]
mod tests;
