// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// Uninterpreted fixed words from a ZIP central-directory record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipHeaderMetadata {
    version_made_by: u16,
    version_needed: u16,
    general_purpose_flags: u16,
    disk_start: u16,
    internal_attributes: u16,
    external_attributes: u32,
}

impl ZipHeaderMetadata {
    pub(crate) const fn new(
        version_made_by: u16,
        version_needed: u16,
        general_purpose_flags: u16,
        disk_start: u16,
        internal_attributes: u16,
        external_attributes: u32,
    ) -> Self {
        Self {
            version_made_by,
            version_needed,
            general_purpose_flags,
            disk_start,
            internal_attributes,
            external_attributes,
        }
    }

    /// Returns the complete version-made-by word, including its host-system byte.
    #[must_use]
    pub const fn raw_version_made_by(&self) -> u16 {
        self.version_made_by
    }

    /// Returns the complete version-needed word without interpreting supported features.
    #[must_use]
    pub const fn raw_version_needed(&self) -> u16 {
        self.version_needed
    }

    /// Returns every stored general-purpose flag bit.
    #[must_use]
    pub const fn general_purpose_flags(&self) -> u16 {
        self.general_purpose_flags
    }

    /// Returns the original 16-bit disk-start word, including a ZIP64 marker.
    #[must_use]
    pub const fn disk_start(&self) -> u16 {
        self.disk_start
    }

    /// Returns all internal-attribute bits without interpretation.
    #[must_use]
    pub const fn internal_attributes(&self) -> u16 {
        self.internal_attributes
    }

    /// Returns all external-attribute bits without host-specific interpretation.
    #[must_use]
    pub const fn external_attributes(&self) -> u32 {
        self.external_attributes
    }
}
