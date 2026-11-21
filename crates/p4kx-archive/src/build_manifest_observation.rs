// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::BuildManifestOrigin;

/// The result of the eager build-manifest read beside a root Archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildManifestOutcome {
    /// The manifest was not found.
    Missing,
    /// The manifest could not be read completely.
    Unreadable,
    /// A completed read parsed as a Build.
    Parsed,
    /// A completed read did not parse as a Build.
    Malformed,
}

/// Cached read outcome with original ownership only for completed reads.
#[derive(Debug, Clone)]
pub struct BuildManifestObservation {
    outcome: BuildManifestOutcome,
    origin: Option<BuildManifestOrigin>,
}

impl BuildManifestObservation {
    pub(crate) const fn missing() -> Self {
        Self {
            outcome: BuildManifestOutcome::Missing,
            origin: None,
        }
    }

    pub(crate) const fn unreadable() -> Self {
        Self {
            outcome: BuildManifestOutcome::Unreadable,
            origin: None,
        }
    }

    pub(crate) fn parsed(byte_length: u64) -> Self {
        Self {
            outcome: BuildManifestOutcome::Parsed,
            origin: Some(BuildManifestOrigin::new(byte_length)),
        }
    }

    pub(crate) fn malformed(byte_length: u64) -> Self {
        Self {
            outcome: BuildManifestOutcome::Malformed,
            origin: Some(BuildManifestOrigin::new(byte_length)),
        }
    }

    /// Returns the original read outcome without reading the manifest again.
    #[must_use]
    pub const fn outcome(&self) -> BuildManifestOutcome {
        self.outcome
    }

    /// Returns the actual completed-read owner, including for malformed content.
    #[must_use]
    pub const fn origin(&self) -> Option<&BuildManifestOrigin> {
        self.origin.as_ref()
    }
}
