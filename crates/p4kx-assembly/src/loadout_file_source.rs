// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_archive::{ArchiveInputOrigin, EntryOrigin};

use crate::LoadoutFileIdentity;

/// The original admission supplying a loadout file operation.
#[derive(Debug, Clone)]
pub struct LoadoutFileSource {
    input: Input,
}

#[derive(Debug, Clone)]
enum Input {
    Archive(EntryOrigin),
    Unbound(Arc<str>),
    MissingArchive {
        path: Arc<str>,
        input: ArchiveInputOrigin,
    },
}

impl LoadoutFileSource {
    pub(crate) fn unbound(path: &str) -> Self {
        Self {
            input: Input::Unbound(Arc::from(path)),
        }
    }

    pub(crate) fn missing_archive(path: &str, input: &ArchiveInputOrigin) -> Self {
        Self {
            input: Input::MissingArchive {
                path: Arc::from(path),
                input: input.clone(),
            },
        }
    }

    /// Returns whether an Archive lookup found no Entry.
    #[must_use]
    pub const fn is_missing_archive(&self) -> bool {
        matches!(self.input, Input::MissingArchive { .. })
    }

    pub(crate) const fn archive(origin: EntryOrigin) -> Self {
        Self {
            input: Input::Archive(origin),
        }
    }

    /// Compares the original admission handles independently of descriptive coordinates.
    #[must_use]
    pub fn is_same_admission(&self, other: &Self) -> bool {
        match (&self.input, &other.input) {
            (Input::Archive(left), Input::Archive(right)) => {
                left.binding().is_same_entry(&right.binding())
            }
            (Input::Unbound(left), Input::Unbound(right)) => Arc::ptr_eq(left, right),
            (
                Input::MissingArchive {
                    path: left,
                    input: left_input,
                },
                Input::MissingArchive {
                    path: right,
                    input: right_input,
                },
            ) => Arc::ptr_eq(left, right) && left_input.is_same_input(right_input),
            (Input::Archive(_), Input::Unbound(_) | Input::MissingArchive { .. })
            | (Input::Unbound(_) | Input::MissingArchive { .. }, Input::Archive(_))
            | (Input::Unbound(_), Input::MissingArchive { .. })
            | (Input::MissingArchive { .. }, Input::Unbound(_)) => false,
        }
    }

    /// Returns descriptive coordinates without reconstructing input authority.
    #[must_use]
    pub fn identity(&self) -> LoadoutFileIdentity<'_> {
        match &self.input {
            Input::Archive(origin) => LoadoutFileIdentity {
                entry_index: Some(origin.entry().index()),
                path: origin.entry().path(),
            },
            Input::Unbound(path) | Input::MissingArchive { path, .. } => LoadoutFileIdentity {
                entry_index: None,
                path,
            },
        }
    }

    /// Borrows the actual Archive input of a present or missing Archive lookup.
    #[must_use]
    pub const fn archive_input_origin(&self) -> Option<&ArchiveInputOrigin> {
        match &self.input {
            Input::Archive(origin) => Some(origin.input_origin()),
            Input::MissingArchive { input, .. } => Some(input),
            Input::Unbound(_) => None,
        }
    }

    /// Borrows the actual immutable Archive admission, when one was observed.
    #[must_use]
    pub const fn entry_origin(&self) -> Option<&EntryOrigin> {
        match &self.input {
            Input::Archive(origin) => Some(origin),
            Input::Unbound(_) | Input::MissingArchive { .. } => None,
        }
    }
}

#[cfg(test)]
#[path = "loadout_file_source_tests.rs"]
mod tests;
