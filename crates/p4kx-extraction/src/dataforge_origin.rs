// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, EntryOrigin, SourceEncryption};

use crate::DataForgeFileOrigin;

/// Retained physical input of successfully read DataForge bytes.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum DataForgeOrigin {
    /// An exact observed Archive input, Entry and containing-Entry context.
    Archive(EntryOrigin),
    /// An observed standalone file without trusted original P4K provenance.
    File(DataForgeFileOrigin),
    /// A generic stream without a trusted physical input binding.
    Unknown,
}

impl DataForgeOrigin {
    /// Returns whether Archive evidence matches the exact input and containing position.
    /// Files and unbound streams return false and require their own input-kind handling.
    #[must_use]
    pub fn matches_archive(&self, archive: &Archive) -> bool {
        match self {
            Self::Archive(origin) => archive.owns_origin(origin),
            Self::File(_) | Self::Unknown => false,
        }
    }

    /// Returns source encryption evidence independently of decoded DataForge bytes.
    #[must_use]
    pub fn encryption(&self) -> SourceEncryption {
        match self {
            Self::Archive(origin) => origin.encryption(),
            Self::File(_) | Self::Unknown => SourceEncryption::Unknown,
        }
    }
}
