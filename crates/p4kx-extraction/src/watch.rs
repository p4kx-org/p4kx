// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use p4kx_archive::{Archive, Build};
use thiserror::Error;

/// How often [`ArchiveWatch::poll`] reads the disk unless set otherwise.
pub const DEFAULT_WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// The Archive changed on disk after it was opened, e.g. because the launcher patched it.
#[derive(Debug, Clone, Error)]
#[error("{} changed on disk after it was opened; open it again", path.display())]
pub struct ArchiveChanged {
    path: PathBuf,
}

impl ArchiveChanged {
    pub(crate) fn of(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    /// Returns the path of the Archive.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// What identifies a version of a file on disk: its size and modification time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl FileStamp {
    /// Returns the stamp of the file at `path` as it is now.
    ///
    /// # Errors
    ///
    /// Returns the error of looking at the file.
    pub fn of(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        Ok(Self::from_metadata(&metadata))
    }

    pub(crate) fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
        }
    }

    /// Returns the size of the file in bytes.
    #[must_use]
    pub const fn size(&self) -> u64 {
        self.len
    }

    /// Returns the modification time, or `None` when the platform gives none.
    #[must_use]
    pub const fn modified(&self) -> Option<SystemTime> {
        self.modified
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Stamp {
    file: FileStamp,
    build: Option<Build>,
}

/// Watches an open Archive for a change of its size, its modification time or its Build.
///
/// The state the Archive was opened in is compared with the disk. Once a change is seen it is
/// kept, even if the file is restored: only opening the Archive again clears it.
#[derive(Debug)]
pub struct ArchiveWatch {
    path: PathBuf,
    opened: Stamp,
    watches_build: bool,
    interval: Duration,
    last_check: Mutex<Option<Instant>>,
    changed: AtomicBool,
}

impl ArchiveWatch {
    /// Starts watching `archive` against the state its file was opened in, checking at most
    /// every [`DEFAULT_WATCH_INTERVAL`] in [`poll`](Self::poll).
    #[must_use]
    pub fn new(archive: &Archive) -> Self {
        Self {
            path: archive.path().to_path_buf(),
            opened: Stamp {
                file: FileStamp {
                    len: archive.file_len(),
                    modified: archive.file_modified(),
                },
                build: archive.build().cloned(),
            },
            watches_build: true,
            interval: DEFAULT_WATCH_INTERVAL,
            last_check: Mutex::new(None),
            changed: AtomicBool::new(false),
        }
    }

    /// Starts watching the plain file at `path`, such as a DataForge file, against its size and
    /// modification time now; no Build belongs to it.
    ///
    /// # Errors
    ///
    /// Returns the error of looking at the file.
    pub fn of_file(path: &Path) -> io::Result<Self> {
        Ok(Self {
            path: path.to_path_buf(),
            opened: Stamp {
                file: FileStamp::of(path)?,
                build: None,
            },
            watches_build: false,
            interval: DEFAULT_WATCH_INTERVAL,
            last_check: Mutex::new(None),
            changed: AtomicBool::new(false),
        })
    }

    /// Makes [`poll`](Self::poll) read the disk at most every `interval`.
    #[must_use]
    pub const fn with_interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    /// Compares the Archive on disk with the state it was opened in.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveChanged`] when the Archive changed now or at an earlier check, or its
    /// file can no longer be read.
    pub fn check(&self) -> Result<(), ArchiveChanged> {
        *self
            .last_check
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
        self.compare();
        self.result()
    }

    /// Checks like [`check`](Self::check) once the interval passed since the last check, and
    /// otherwise returns what is known.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveChanged`] when a change was seen.
    pub fn poll(&self) -> Result<(), ArchiveChanged> {
        let due = {
            let Ok(mut last) = self.last_check.try_lock() else {
                return self.result();
            };
            let now = Instant::now();
            let due = last.is_none_or(|last| now.duration_since(last) >= self.interval);
            if due {
                *last = Some(now);
            }
            due
        };
        if due {
            self.compare();
        }
        self.result()
    }

    /// Returns whether a change was seen, without reading the disk.
    #[must_use]
    pub fn is_changed(&self) -> bool {
        self.changed.load(Ordering::Relaxed)
    }

    /// Compares only the size and modification time of the file with the disk, which is cheaper
    /// than a full check; returns whether a change was seen now or before.
    pub(crate) fn file_changed(&self) -> bool {
        if !self.is_changed() {
            let unchanged = FileStamp::of(&self.path).is_ok_and(|now| now == self.opened.file);
            if !unchanged {
                self.changed.store(true, Ordering::Relaxed);
            }
        }
        self.is_changed()
    }

    fn compare(&self) {
        if !self.is_changed() && self.read_stamp().as_ref() != Some(&self.opened) {
            self.changed.store(true, Ordering::Relaxed);
        }
    }

    fn result(&self) -> Result<(), ArchiveChanged> {
        if self.is_changed() {
            Err(ArchiveChanged {
                path: self.path.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn read_stamp(&self) -> Option<Stamp> {
        let file = FileStamp::of(&self.path).ok()?;
        let build = if self.watches_build {
            self.path
                .parent()
                .and_then(|folder| Build::read_from(folder).ok().flatten())
        } else {
            None
        };
        Some(Stamp { file, build })
    }
}
