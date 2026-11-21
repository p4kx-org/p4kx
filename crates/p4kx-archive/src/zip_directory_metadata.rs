// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::cig_install_metadata::CigInstallMetadata;
use crate::metadata_range::MetadataRange;
use crate::metadata_region::MetadataRegionStatus;
use crate::metadata_tables::MetadataTables;
use crate::zip_end_metadata::ZipEndMetadata;
use crate::zip64_end_metadata::Zip64EndMetadata;
use crate::zip64_locator_metadata::Zip64LocatorMetadata;

/// Selected ZIP directory and original end-record declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZipDirectoryMetadata {
    pub(crate) records: MetadataRange,
    pub(crate) count: u64,
    pub(crate) end: ZipEndMetadata,
    pub(crate) zip64: Option<Zip64EndMetadata>,
    pub(crate) locator: Option<Zip64LocatorMetadata>,
    pub(crate) installation: Option<CigInstallMetadata>,
}

impl ZipDirectoryMetadata {
    /// Returns the selected central-directory bytes.
    #[must_use]
    pub const fn records(&self) -> MetadataRange {
        self.records
    }
    /// Returns the declared original directory count before admission.
    #[must_use]
    pub const fn count(&self) -> u64 {
        self.count
    }
    /// Returns every original classic end-record word.
    #[must_use]
    pub const fn end(&self) -> &ZipEndMetadata {
        &self.end
    }
    /// Returns the selected ZIP64 end record, absent for ordinary ZIP.
    #[must_use]
    pub const fn zip64(&self) -> Option<&Zip64EndMetadata> {
        self.zip64.as_ref()
    }
    /// Returns the selected locator, including its actual physical source position.
    #[must_use]
    pub const fn locator(&self) -> Option<&Zip64LocatorMetadata> {
        self.locator.as_ref()
    }
    /// Returns positively recognised CIG installation declarations.
    #[must_use]
    pub const fn installation(&self) -> Option<&CigInstallMetadata> {
        self.installation.as_ref()
    }

    pub(crate) fn is_complete(&self) -> bool {
        if self.end.comment().status() != MetadataRegionStatus::Complete {
            return false;
        }
        if let Some(zip64) = self.zip64 {
            let Some(extension) = zip64.extension() else {
                return false;
            };
            if extension.status() != MetadataRegionStatus::Complete {
                return false;
            }
            if let (Some(range), Some(locator)) = (extension.available(), self.locator)
                && range.end() > locator.range().offset()
            {
                return false;
            }
        }
        !self
            .installation
            .is_some_and(|value| matches!(value.tables(), MetadataTables::Invalid(_)))
    }
}
