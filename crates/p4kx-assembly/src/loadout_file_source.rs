// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_archive::EntryOrigin;

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
}

impl LoadoutFileSource {
    pub(crate) fn unbound(path: &str) -> Self {
        Self {
            input: Input::Unbound(Arc::from(path)),
        }
    }

    pub(crate) const fn archive(origin: EntryOrigin) -> Self {
        Self {
            input: Input::Archive(origin),
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
            Input::Unbound(path) => LoadoutFileIdentity {
                entry_index: None,
                path,
            },
        }
    }

    /// Borrows the actual immutable Archive admission, when one was observed.
    #[must_use]
    pub const fn entry_origin(&self) -> Option<&EntryOrigin> {
        match &self.input {
            Input::Archive(origin) => Some(origin),
            Input::Unbound(_) => None,
        }
    }
}
