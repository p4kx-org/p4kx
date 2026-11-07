// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::SystemTime;

use p4kx_export::{
    CancelToken, CatalogError, CatalogFile, CatalogOptions, CatalogProgress,
    CatalogPublicationMode, CatalogSummary, Compression, ExportHealth, write_catalog_file_from,
};
use p4kx_extraction::{ArchiveWriteProtection, is_same_file};

use super::{
    CatalogExportOptions, CatalogExportProgress, CatalogExportStage, CatalogExportSummary, Shared,
};
use crate::EntryIndex;
use crate::forge_cache::ReadFailure;
use crate::model::describe;

/// Where a Basetool catalog stands.
#[derive(Debug, Clone, Copy)]
pub enum CatalogExportStatus<'a> {
    /// The catalog is being created; the progress so far.
    Running(CatalogExportProgress),
    /// The catalog file was written.
    Finished(&'a CatalogExportSummary),
    /// The catalog was cancelled; no file was written.
    Cancelled,
    /// The chosen file is the open Archive; nothing was written.
    IsArchive,
    /// The chosen file ends in `.gz`, which is not written; nothing was written.
    GzipNotSupported,
    /// Selected data could not be read or projected; normal publication wrote nothing.
    Incomplete(&'a ExportHealth),
    /// The catalog could not be created, for the reason given, or it stopped without a result.
    Failed(Option<&'a str>),
}

enum Outcome {
    Written(Box<CatalogExportSummary>),
    Incomplete(Box<ExportHealth>),
    Cancelled,
    IsArchive,
}

type RunResult = Result<Outcome, String>;

#[derive(Debug)]
enum State {
    Running(Receiver<RunResult>),
    Finished(Box<CatalogExportSummary>),
    Cancelled,
    IsArchive,
    GzipNotSupported,
    Incomplete(Box<ExportHealth>),
    Failed(Option<String>),
}

/// A Basetool catalog created on a background thread, or its outcome.
#[derive(Debug)]
pub struct CatalogExport {
    id: u64,
    file: PathBuf,
    shown: String,
    cancel: CancelToken,
    shared: Arc<Shared>,
    publication_mode: CatalogPublicationMode,
    state: State,
}

impl CatalogExport {
    fn new(
        file: PathBuf,
        cancel: CancelToken,
        shared: Arc<Shared>,
        state: State,
        publication_mode: CatalogPublicationMode,
    ) -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            shown: file.display().to_string(),
            file,
            cancel,
            shared,
            publication_mode,
            state,
        }
    }

    /// Returns the number that tells this catalog apart from every other one of the process.
    #[must_use]
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Returns the catalog file chosen.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the catalog file chosen as the windows show it.
    #[must_use]
    pub fn shown_file(&self) -> &str {
        &self.shown
    }

    /// Returns the publication policy explicitly selected for this job.
    #[must_use]
    pub const fn publication_mode(&self) -> CatalogPublicationMode {
        self.publication_mode
    }

    /// Returns where the catalog stands.
    #[must_use]
    pub fn status(&self) -> CatalogExportStatus<'_> {
        match &self.state {
            State::Running(_) => CatalogExportStatus::Running(self.shared.progress()),
            State::Finished(summary) => CatalogExportStatus::Finished(summary),
            State::Cancelled => CatalogExportStatus::Cancelled,
            State::IsArchive => CatalogExportStatus::IsArchive,
            State::GzipNotSupported => CatalogExportStatus::GzipNotSupported,
            State::Incomplete(health) => CatalogExportStatus::Incomplete(health),
            State::Failed(reason) => CatalogExportStatus::Failed(reason.as_deref()),
        }
    }

    /// Returns whether the catalog is still being created.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self.state, State::Running(_))
    }

    /// Returns whether the catalog was asked to stop.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    /// Asks the catalog to stop; no file is written then.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Takes the outcome once the catalog ended; returns whether it ended.
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

    /// Cancels the catalog and waits until it ended, its temporary file removed.
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

    /// Ends a cancelled catalog at once while it still reads DataForge, which writes nothing;
    /// the thread finishes reading and keeps DataForge for the next export. Returns whether it
    /// ended.
    fn give_up(&mut self) -> bool {
        if !matches!(self.state, State::Running(_)) || !self.shared.gate.give_up() {
            return false;
        }
        self.state = State::Cancelled;
        true
    }
}

fn ended(outcome: Option<RunResult>) -> State {
    match outcome {
        Some(Ok(Outcome::Written(summary))) => State::Finished(summary),
        Some(Ok(Outcome::Cancelled)) => State::Cancelled,
        Some(Ok(Outcome::IsArchive)) => State::IsArchive,
        Some(Ok(Outcome::Incomplete(health))) => State::Incomplete(health),
        Some(Err(reason)) => State::Failed(Some(reason)),
        None => State::Failed(None),
    }
}

/// Creates the Basetool catalog of the Archive of `index` into `file` on a background thread,
/// reading DataForge into the cache of `index` unless it holds it already, and calls `wake` from
/// that thread once the catalog ended.
pub(crate) fn start(
    index: &Arc<EntryIndex>,
    file: PathBuf,
    options: CatalogExportOptions,
    wake: impl Fn() + Send + 'static,
    #[cfg(feature = "test-hooks")] pause: crate::hold::Pause,
) -> CatalogExport {
    let cancel = CancelToken::new();
    let shared = Arc::new(Shared::default());
    let (sender, result) = mpsc::channel();
    let publication_mode = options.publication_mode;
    let job = Job {
        index: Arc::clone(index),
        file: file.clone(),
        cancel: cancel.clone(),
        shared: Arc::clone(&shared),
        options,
        #[cfg(feature = "test-hooks")]
        pause,
    };
    let spawned = thread::Builder::new()
        .name("basetool-catalog".to_owned())
        .spawn(move || {
            if sender.send(job.run()).is_ok() {
                wake();
            }
        });
    let state = match spawned {
        Ok(_handle) => State::Running(result),
        Err(error) => State::Failed(Some(describe(&error))),
    };
    CatalogExport::new(file, cancel, shared, state, publication_mode)
}

/// Returns the outcome of a catalog refused because `file` is the open Archive.
pub(crate) fn refused(file: PathBuf, mode: CatalogPublicationMode) -> CatalogExport {
    CatalogExport::new(
        file,
        CancelToken::new(),
        Arc::default(),
        State::IsArchive,
        mode,
    )
}

/// Returns the outcome of a catalog refused because `file` ends in `.gz`.
pub(crate) fn refused_gzip(file: PathBuf, mode: CatalogPublicationMode) -> CatalogExport {
    CatalogExport::new(
        file,
        CancelToken::new(),
        Arc::default(),
        State::GzipNotSupported,
        mode,
    )
}

struct Job {
    index: Arc<EntryIndex>,
    file: PathBuf,
    cancel: CancelToken,
    shared: Arc<Shared>,
    options: CatalogExportOptions,
    #[cfg(feature = "test-hooks")]
    pause: crate::hold::Pause,
}

impl Job {
    fn run(&self) -> RunResult {
        if is_same_file(&self.file, self.index.path()) {
            return Ok(Outcome::IsArchive);
        }
        let archive = self.index.entries_archive().map_err(|e| e.to_string())?;
        let source = self
            .index
            .forge()
            .read(&self.index, || {})
            .map_err(ReadFailure::into_reason)?;
        if !self.shared.gate.leave_reading() {
            return Ok(Outcome::Cancelled);
        }
        self.shared.enter(CatalogExportStage::Building);
        #[cfg(feature = "test-hooks")]
        self.pause.wait(|| self.cancel.is_cancelled());
        let progress = |progress: CatalogProgress| self.shared.report(progress);
        let patches = crate::public_patches::current();
        let options = CatalogOptions {
            filter: self.options.filter.as_ref(),
            exported_at: SystemTime::now(),
            cancel: Some(&self.cancel),
            progress: Some(&progress),
            public_patches: Some(&patches),
            publication_mode: self.options.publication_mode,
        };
        let protection = match ArchiveWriteProtection::new(archive.path(), self.cancel.flag()) {
            Ok(protection) => protection,
            Err(error) if error.is_cancelled() => return Ok(Outcome::Cancelled),
            Err(error) => return Err(describe(&error)),
        };
        let created = write_catalog_file_from(
            archive,
            &source.entry,
            &options,
            Compression::for_path(&self.file),
            || {
                self.shared.enter(CatalogExportStage::Writing);
                CatalogFile::create_protected(&self.file, &protection, &self.cancel)
            },
        );
        let (summary, file) = match created {
            Ok(created) => created,
            Err(CatalogError::Cancelled) => return Ok(Outcome::Cancelled),
            Err(error) if crate::installation::guard_cancelled(&error) => {
                return Ok(Outcome::Cancelled);
            }
            Err(CatalogError::IncompleteExport { health }) => {
                return Ok(Outcome::Incomplete(health));
            }
            Err(error) => return Err(describe(&error)),
        };
        self.complete(summary, file)
    }

    fn complete(&self, summary: CatalogSummary, file: CatalogFile<'_>) -> RunResult {
        if let Err(source) = self.index.watch().check() {
            let primary = CatalogError::Write(std::io::Error::other(source));
            return Err(describe(&super::output::discard(file, primary)));
        }
        if self.cancel.is_cancelled() {
            return match super::output::discard(file, CatalogError::Cancelled) {
                CatalogError::Cancelled => Ok(Outcome::Cancelled),
                error => Err(describe(&error)),
            };
        }
        file.commit().map_err(|error| describe(&error))?;
        Ok(Outcome::Written(Box::new(CatalogExportSummary {
            file: self.file.clone(),
            summary,
        })))
    }
}
