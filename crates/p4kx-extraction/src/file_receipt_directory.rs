// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use same_file::Handle;

use super::directory_table;
use super::directory_wire::{self, CommittedArtifact};
use super::header::{self, ReceiptOwner};
use super::lock::{ReceiptLock, invalid, regular_file};
use super::{DIRECTORY_NAME, check, sibling_receipt, target};
use crate::origin_file_io;
use crate::output_file::{self, write_error};
use crate::{
    ArchiveWriteProtection, CancelToken, ConversionError, FieldOrigin, OriginArtifactName,
    OriginDigestWriter, OriginOutputAddress, SourceInventory,
};

/// Owns one directory's artifact and origin publication session.
pub struct DirectoryReceipt<'source> {
    directory: PathBuf,
    receipt: PathBuf,
    lock: ReceiptLock,
    protection: Option<&'source ArchiveWriteProtection>,
    cancel: CancelToken,
    state: Mutex<PublicationState>,
    identities: RandomState,
}

struct Reservation {
    path: PathBuf,
    name: OriginArtifactName,
    prior: Option<Handle>,
}

struct PublicationState {
    previous: bool,
    requested: HashSet<PathBuf>,
    active: usize,
    committed: usize,
    committed_identities: HashMap<u64, Vec<PathBuf>>,
    finished: bool,
}

impl PublicationState {
    fn release(&mut self, actual: &Path, committed: bool) {
        self.active -= 1;
        if committed {
            self.committed += 1;
        } else {
            self.requested.remove(actual);
        }
    }
}

struct CommitGuard<'session> {
    state: MutexGuard<'session, PublicationState>,
    staged_identity_hash: u64,
}

impl CommitGuard<'_> {
    fn record(mut self, actual: &Path) {
        self.state
            .committed_identities
            .entry(self.staged_identity_hash)
            .or_default()
            .push(actual.to_owned());
        self.state.release(actual, true);
    }
}

/// Proof that one artifact was committed by the owning directory session.
pub struct DirectoryCommit<'owner> {
    lock: &'owner ReceiptLock,
    artifact: CommittedArtifact,
}

impl DirectoryCommit<'_> {
    /// Returns the byte count measured while writing the artifact.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.artifact.digest.byte_length()
    }
}

impl<'source> DirectoryReceipt<'source> {
    /// Acquires the directory's publication lock and validates its existing companion.
    ///
    /// # Errors
    ///
    /// Returns output protection, directory lock or ownership errors.
    pub fn new(
        directory: &Path,
        protection: Option<&'source ArchiveWriteProtection>,
    ) -> io::Result<Self> {
        let directory = fs::canonicalize(directory)?;
        let receipt = directory.join(DIRECTORY_NAME);
        let cancel = CancelToken::new();
        check(&receipt, protection, &cancel)?;
        let lock = ReceiptLock::acquire(&directory, protection, &cancel)?;
        let previous = directory_table::lookup(&receipt, None)?.is_some();
        Ok(Self {
            directory,
            receipt,
            lock,
            protection,
            cancel,
            state: Mutex::new(PublicationState {
                previous,
                requested: HashSet::new(),
                active: 0,
                committed: 0,
                committed_identities: HashMap::new(),
                finished: false,
            }),
            identities: RandomState::new(),
        })
    }

    /// Returns the canonical directory companion destination.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.receipt
    }

    /// Commits one measured artifact through the protected output writer.
    ///
    /// # Errors
    ///
    /// Returns artifact, write, output protection or origin invalidation errors.
    pub fn write<'owner, T>(
        &'owner self,
        basename: &OsStr,
        write: impl FnOnce(&mut dyn Write) -> Result<T, ConversionError>,
    ) -> Result<(T, DirectoryCommit<'owner>), ConversionError> {
        let artifact = self.directory.join(basename);
        let reservation = self
            .reserve_artifact(basename)
            .map_err(|source| write_error(&artifact, source))?;
        let actual = &reservation.path;
        let written = output_file::write_replacing_protected_before(
            actual,
            self.protection,
            Some(&self.cancel),
            |writer| {
                let mut measured = OriginDigestWriter::new(writer);
                let result = write(&mut measured)?;
                let digest = measured
                    .finish()
                    .map_err(|source| write_error(actual, source))?;
                Ok((result, digest))
            },
            |staged| {
                self.before_replace(&reservation, staged.temporary())
                    .map_err(|source| write_error(actual, source))
            },
        );
        let (result, digest) = match written {
            Ok((artifact, commit)) => {
                commit.record(actual);
                artifact
            }
            Err(error) => {
                self.state()
                    .map_err(|source| write_error(actual, source))?
                    .release(actual, false);
                return Err(error);
            }
        };
        Ok((
            result,
            DirectoryCommit {
                lock: &self.lock,
                artifact: CommittedArtifact {
                    name: reservation.name,
                    digest,
                },
            },
        ))
    }

    /// Publishes the origin companion after every committed artifact has supplied its evidence.
    ///
    /// # Errors
    ///
    /// Returns incomplete, concurrent or foreign commit, serialization, or publication errors.
    pub fn publish<'owner>(
        &'owner self,
        commits: impl IntoIterator<Item = DirectoryCommit<'owner>>,
        inventory: &SourceInventory,
        fields: impl IntoIterator<Item = (usize, OriginOutputAddress, FieldOrigin)>,
    ) -> io::Result<bool> {
        let artifacts = commits
            .into_iter()
            .map(|commit| {
                if !std::ptr::eq(commit.lock, std::ptr::from_ref(&self.lock)) {
                    return Err(invalid(
                        "artifact belongs to a different directory publication",
                    ));
                }
                Ok(commit.artifact)
            })
            .collect::<io::Result<Vec<_>>>()?;
        {
            let mut state = self.state()?;
            if state.finished || state.active != 0 || state.committed != artifacts.len() {
                return Err(invalid(
                    "the directory publication has unfinished or unreported artifacts",
                ));
            }
            state.finished = true;
        }
        if artifacts.is_empty() {
            return Ok(false);
        }
        origin_file_io::write_replacing_before(
            &self.receipt,
            self.protection,
            Some(&self.cancel),
            |writer| directory_wire::write(writer, &artifacts, inventory, fields),
            |_| {
                self.lock.check(&self.cancel)?;
                if regular_file(&self.receipt)?.is_some() {
                    return Err(invalid("a directory companion appeared during publication"));
                }
                Ok(())
            },
        )?;
        Ok(true)
    }

    fn reserve_artifact(&self, basename: &OsStr) -> io::Result<Reservation> {
        OriginArtifactName::from_basename(basename)?;
        let actual = target(
            &self.directory.join(basename),
            self.protection,
            &self.cancel,
        )?;
        if actual.parent() != Some(self.directory.as_path()) {
            return Err(invalid("the artifact is outside its bound directory"));
        }
        let name = OriginArtifactName::from_basename(
            actual
                .file_name()
                .ok_or_else(|| invalid("the artifact has no basename"))?,
        )?;
        let mut state = self.state()?;
        if state.finished {
            return Err(invalid("the directory publication is finished"));
        }
        let prior = artifact_identity(&actual)?;
        if let Some(identity) = &prior {
            self.check_new_identity(&state, identity)?;
        }
        if !state.requested.insert(actual.clone()) {
            return Err(invalid(
                "an artifact was requested twice in one directory publication",
            ));
        }
        state.active += 1;
        drop(state);
        Ok(Reservation {
            path: actual,
            name,
            prior,
        })
    }

    fn check_new_identity(&self, state: &PublicationState, identity: &Handle) -> io::Result<()> {
        if let Some(candidates) = state
            .committed_identities
            .get(&self.identities.hash_one(identity))
        {
            for candidate in candidates {
                if artifact_identity(candidate)?.as_ref() == Some(identity) {
                    return Err(invalid(
                        "the artifact aliases another commit in this directory",
                    ));
                }
            }
        }
        Ok(())
    }

    fn state(&self) -> io::Result<MutexGuard<'_, PublicationState>> {
        self.state
            .lock()
            .map_err(|_| io::Error::other("the artifact publication was interrupted"))
    }

    fn before_replace(
        &self,
        reservation: &Reservation,
        staged: &Path,
    ) -> io::Result<CommitGuard<'_>> {
        let mut state = self.state()?;
        self.lock.check(&self.cancel)?;
        let actual = &reservation.path;
        if target(actual, self.protection, &self.cancel)? != *actual
            || artifact_identity(actual)? != reservation.prior
        {
            return Err(invalid("the artifact changed after it was reserved"));
        }
        let staged_identity =
            artifact_identity(staged)?.ok_or_else(|| invalid("the staged artifact disappeared"))?;
        let staged_identity_hash = self.identities.hash_one(&staged_identity);
        let sibling = sibling_receipt(actual)?;
        check(&sibling, self.protection, &self.cancel)?;
        let sibling_exists =
            header::check_existing(&sibling, ReceiptOwner::File(&reservation.name))?;
        if state.previous {
            check(&self.receipt, self.protection, &self.cancel)?;
            if directory_table::lookup(&self.receipt, None)?.is_some() {
                fs::remove_file(&self.receipt)?;
            }
            state.previous = false;
        } else if regular_file(&self.receipt)?.is_some() {
            return Err(invalid("a directory companion appeared during publication"));
        }
        if sibling_exists {
            fs::remove_file(sibling)?;
        }
        Ok(CommitGuard {
            state,
            staged_identity_hash,
        })
    }
}

fn artifact_identity(path: &Path) -> io::Result<Option<Handle>> {
    if regular_file(path)?.is_some() {
        Handle::from_path(path).map(Some)
    } else {
        Ok(None)
    }
}
