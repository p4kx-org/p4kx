// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_foundation::ArchiveKey;

use super::Archive;
use crate::key_check::{self, Verdict};
use crate::{ArchiveError, Entry, KeyCheck, KeyDemand, OpenOptions};

impl Archive {
    /// Opens the Archive at `path` like [`Archive::open`] and sets `key`, when given, as the key
    /// of its encrypted Entries; see [`Archive::set_key`].
    ///
    /// # Errors
    ///
    /// Returns an [`ArchiveError`] when the file cannot be read or is not a valid Archive.
    pub fn open_with_key(
        path: impl AsRef<Path>,
        key: Option<ArchiveKey>,
    ) -> Result<Self, ArchiveError> {
        Self::open_with(path, OpenOptions::new().key(key))
    }

    /// Sets the key that decrypts the Entries of this Archive and of the containers opened from
    /// it, also those opened already.
    ///
    /// The key is checked once, when an encrypted Entry is first opened with it: a key that does
    /// not decrypt the smallest suitable encrypted Entry to data matching its checksum makes
    /// opening fail with [`ArchiveError::WrongKey`], from then on without checking again.
    /// [`Archive::check_key`] checks it at once.
    pub fn set_key(&self, key: ArchiveKey) {
        self.key.set(key);
    }

    /// Removes the key; encrypted Entries then fail to open with [`ArchiveError::KeyRequired`].
    pub fn clear_key(&self) {
        self.key.clear();
    }

    /// Returns whether a key is set.
    #[must_use]
    pub fn has_key(&self) -> bool {
        self.key.is_set()
    }

    /// Returns whether an encrypted Entry was needed since the last call while no key was set or
    /// the one set was found not to fit, and forgets that, so that a window can ask for the key
    /// once.
    #[must_use]
    pub fn take_key_request(&self) -> bool {
        self.key.take_wanted()
    }

    /// Returns the handle to the record of the encrypted Entries that were asked for without a key,
    /// or with one that does not fit; unlike [`Archive::take_key_request`] reading it resets nothing.
    ///
    /// Opening an Entry or a container records it; [`Archive::check_key`] does not.
    #[must_use]
    pub fn key_demand(&self) -> KeyDemand {
        self.key.demand().clone()
    }

    /// Checks the key that is set, which is [`KeyCheck::Unchecked`] when none is.
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError::WrongKey`] when the key does not decrypt the Archive's Entries.
    pub fn check_key(&self) -> Result<KeyCheck, ArchiveError> {
        match self.key.get() {
            None => Ok(KeyCheck::Unchecked),
            Some((key, _)) => self.check_set_key(&key),
        }
    }

    /// Checks whether `key` decrypts this Archive's encrypted Entries, by decrypting the smallest
    /// suitable ones (at most three) and comparing the CRC-32C of the result with the directory.
    /// Nothing is set. An Entry that cannot be read leaves the key [`KeyCheck::Unchecked`].
    ///
    /// # Errors
    ///
    /// Returns [`ArchiveError::WrongKey`] when the data of every candidate does not match.
    pub fn verify_key(&self, key: &ArchiveKey) -> Result<KeyCheck, ArchiveError> {
        match key_check::verify(&self.source, &self.entries, key) {
            Verdict::Verified => Ok(KeyCheck::Verified),
            Verdict::Unchecked => Ok(KeyCheck::Unchecked),
            Verdict::Wrong { entry } => Err(ArchiveError::WrongKey {
                path: self.location.clone(),
                entry,
            }),
        }
    }

    fn check_set_key(&self, key: &ArchiveKey) -> Result<KeyCheck, ArchiveError> {
        match self.verify_key(key) {
            Ok(check) => {
                self.key.record(key, Ok(()));
                Ok(check)
            }
            Err(ArchiveError::WrongKey { path, entry }) => {
                self.key.record(key, Err(entry.clone()));
                Err(ArchiveError::WrongKey { path, entry })
            }
            Err(error) => Err(error),
        }
    }

    fn check_once(&self, key: &ArchiveKey) -> Result<(), ArchiveError> {
        let _turn = self.key.check_turn();
        match self.key.get() {
            Some((current, Some(Err(entry)))) if &current == key => Err(ArchiveError::WrongKey {
                path: self.location.clone(),
                entry,
            }),
            Some((current, Some(Ok(())))) if &current == key => Ok(()),
            _ => self.check_set_key(key).map(|_| ()),
        }
    }

    /// Returns the key to decrypt `entry` with, `None` for an unencrypted Entry, and records the
    /// Entry in the key demand when no key fits.
    pub(super) fn key_for(&self, entry: &Entry) -> Result<Option<ArchiveKey>, ArchiveError> {
        let key = self.resolve_key(entry);
        match &key {
            Err(ArchiveError::KeyRequired { .. }) => self.key.demand().record_missing(&entry.path),
            Err(ArchiveError::WrongKey { .. }) => self.key.demand().record_wrong(&entry.path),
            _ => {}
        }
        key
    }

    fn resolve_key(&self, entry: &Entry) -> Result<Option<ArchiveKey>, ArchiveError> {
        if !entry.encrypted {
            return Ok(None);
        }
        let Some((key, outcome)) = self.key.get() else {
            self.key.want();
            return Err(ArchiveError::KeyRequired {
                path: self.location.clone(),
                entry: entry.path.clone(),
            });
        };
        match outcome {
            Some(Ok(())) => {}
            Some(Err(entry)) => {
                return Err(ArchiveError::WrongKey {
                    path: self.location.clone(),
                    entry,
                });
            }
            None => {
                self.check_once(&key)?;
            }
        }
        Ok(Some(key))
    }
}
