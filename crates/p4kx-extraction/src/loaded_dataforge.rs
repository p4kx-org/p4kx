// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use p4kx_archive::Archive;
use p4kx_dataforge::{DataForge, DataForgeError};
use thiserror::Error;

use crate::{ArchiveWatch, DataForgeEntry, DataForgeLoadError, DataForgeOrigin};

type Job = Box<dyn FnOnce(&DataForge<'_>, &DataForgeOrigin) + Send>;

/// The DataForge of an Archive, read once and kept open, with the indexes it builds on first
/// use, for as long as the value lives.
///
/// The DataForge is opened over its bytes on a thread of its own; [`LoadedDataForge::with`]
/// runs work against it there, each call on a new scoped thread, so calls run side by side.
/// Dropping the value ends the thread once the calls running on it are done.
#[derive(Debug)]
pub struct LoadedDataForge {
    path: String,
    origin: DataForgeOrigin,
    jobs: mpsc::Sender<Job>,
}

/// The thread holding a [`LoadedDataForge`] stopped before the work ran to the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("the DataForge stopped before the work ended")]
pub struct DataForgeStopped;

impl LoadedDataForge {
    /// Reads the DataForge Entry of `archive` as [`DataForgeEntry::read`] does and opens it.
    ///
    /// # Errors
    ///
    /// Returns the [`DataForgeLoadError`] of reading the Entry,
    /// [`DataForgeLoadError::Corrupt`] when the DataForge cannot be opened,
    /// [`DataForgeLoadError::ArchiveChanged`] when the Archive changed on disk while it was
    /// read, and [`DataForgeLoadError::Thread`] when no thread can be started.
    pub fn load(archive: &Archive) -> Result<Self, DataForgeLoadError> {
        let watch = ArchiveWatch::new(archive);
        let entry = DataForgeEntry::read(archive)?;
        watch.check().map_err(DataForgeLoadError::ArchiveChanged)?;
        Self::start(entry)
    }

    /// Reads the DataForge file at `file` as [`DataForgeEntry::read_file`] does and opens it.
    ///
    /// # Errors
    ///
    /// Returns the [`DataForgeLoadError`] of reading the file, [`DataForgeLoadError::Corrupt`]
    /// when the DataForge cannot be opened and [`DataForgeLoadError::Thread`] when no thread can
    /// be started.
    pub fn load_file(file: &Path) -> Result<Self, DataForgeLoadError> {
        Self::start(DataForgeEntry::read_file(file)?)
    }

    fn start(entry: DataForgeEntry) -> Result<Self, DataForgeLoadError> {
        let path = entry.path().to_owned();
        let origin = entry.origin().clone();
        let (ready, opened) = mpsc::sync_channel(1);
        let (jobs, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("dataforge".to_owned())
            .spawn(move || serve(&entry, &receiver, &ready))
            .map_err(DataForgeLoadError::Thread)?;
        match opened.recv() {
            Ok(Ok(())) => Ok(Self { path, origin, jobs }),
            Ok(Err(error)) => Err(DataForgeLoadError::Corrupt(error)),
            Err(_) => Err(DataForgeLoadError::Thread(io::Error::other(
                "the DataForge thread ended before it opened the DataForge",
            ))),
        }
    }

    /// Returns the path of the Entry the DataForge was read from.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the physical origin retained by the worker and its cached handle.
    #[must_use]
    pub const fn origin(&self) -> &DataForgeOrigin {
        &self.origin
    }

    /// Runs `work` against the DataForge and returns its result; the caller waits meanwhile.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeStopped`] when the work could not run or ended without a result.
    pub fn with<T, F>(&self, work: F) -> Result<T, DataForgeStopped>
    where
        T: Send + 'static,
        F: FnOnce(&DataForge<'_>) -> T + Send + 'static,
    {
        self.with_origin(move |forge, _origin| work(forge))
    }

    /// Runs work against the DataForge and the worker's retained physical origin.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeStopped`] when work could not run or ended without a result.
    pub fn with_origin<T, F>(&self, work: F) -> Result<T, DataForgeStopped>
    where
        T: Send + 'static,
        F: FnOnce(&DataForge<'_>, &DataForgeOrigin) -> T + Send + 'static,
    {
        let (sender, result) = mpsc::sync_channel(1);
        let job: Job = Box::new(move |forge, origin| {
            let _ignored = sender.send(work(forge, origin));
        });
        self.jobs.send(job).map_err(|_| DataForgeStopped)?;
        result.recv().map_err(|_| DataForgeStopped)
    }
}

fn serve(
    entry: &DataForgeEntry,
    jobs: &Receiver<Job>,
    ready: &SyncSender<Result<(), DataForgeError>>,
) {
    let forge = match entry.open() {
        Ok(forge) => forge,
        Err(error) => {
            let _ignored = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(())).is_err() {
        return;
    }
    let forge = &forge;
    let origin = entry.origin();
    thread::scope(|scope| {
        for job in jobs {
            let _ignored = thread::Builder::new()
                .name("dataforge-call".to_owned())
                .spawn_scoped(scope, move || job(forge, origin));
        }
    });
}
