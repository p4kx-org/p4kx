// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Source;

/// The actual closed payload carrier shape, without serialized-identity authority.
#[derive(Clone, Copy)]
pub(crate) enum OriginSubject<'a> {
    Record(&'a Source),
    Source(&'a Source),
    Artifact,
    IndexBuild,
}

impl<'a> OriginSubject<'a> {
    pub(crate) const fn source_data(self) -> Option<&'a Source> {
        match self {
            Self::Record(source) | Self::Source(source) => Some(source),
            Self::Artifact | Self::IndexBuild => None,
        }
    }
}
