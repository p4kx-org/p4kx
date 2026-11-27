// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::OnceLock;

use p4kx_archive::Archive;
use p4kx_extraction::CancelToken;

use super::AudioIndex;

/// The audio of one open Archive: its [`AudioIndex`] is built on the first use and kept, or is
/// one the caller built and keeps.
#[derive(Debug)]
pub struct ArchiveAudio<'a> {
    archive: &'a Archive,
    index: OnceLock<AudioIndex>,
    shared: Option<&'a AudioIndex>,
}

impl<'a> ArchiveAudio<'a> {
    /// Reads audio from `archive`; nothing is read before the first [`index`](Self::index).
    #[must_use]
    pub const fn new(archive: &'a Archive) -> Self {
        Self {
            archive,
            index: OnceLock::new(),
            shared: None,
        }
    }

    /// Reads audio from `archive` with `index`, which the caller built from that Archive with
    /// [`AudioIndex::build`] and keeps, so that it is not built again.
    #[must_use]
    pub const fn with_index(archive: &'a Archive, index: &'a AudioIndex) -> Self {
        Self {
            archive,
            index: OnceLock::new(),
            shared: Some(index),
        }
    }

    /// Returns the Archive.
    #[must_use]
    pub const fn archive(&self) -> &'a Archive {
        self.archive
    }

    /// Returns the audio index, building it on the first call; later calls, from any thread,
    /// return the same index. Callers racing the first call may each build one; the first
    /// stored wins and the others are dropped.
    pub fn index(&self) -> &AudioIndex {
        if let Some(index) = self.cached_index() {
            return index;
        }
        let built = AudioIndex::build(self.archive);
        self.index.get_or_init(|| built)
    }

    /// Returns the index, or stops and returns `None` when the caller cancels.
    ///
    /// A cancelled build is discarded so the next call can build the complete index.
    #[must_use]
    pub fn index_until(&self, cancel: &CancelToken) -> Option<&AudioIndex> {
        if cancel.is_cancelled() {
            return None;
        }
        let index = if let Some(index) = self.cached_index() {
            index
        } else {
            let built = AudioIndex::build_until(self.archive, cancel)?;
            if cancel.is_cancelled() {
                return None;
            }
            self.index.get_or_init(|| built)
        };
        (!cancel.is_cancelled()).then_some(index)
    }

    fn cached_index(&self) -> Option<&AudioIndex> {
        self.shared.or_else(|| self.index.get())
    }
}
