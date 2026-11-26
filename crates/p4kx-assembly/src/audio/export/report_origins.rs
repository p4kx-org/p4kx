// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, FieldOrigin, SourceEvidenceError, SourceUses};

use super::ExportedSound;

/// Additional evidence requested from a sound export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportReportCapture {
    /// Retain the exact original inputs to measured file sizes.
    FileSizes,
    /// Retain file size inputs and the original WEM loop inputs for TXTP generation.
    FileSizesAndLoops,
}

/// The resolved input positions used by a planned sound file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportSoundSelection {
    /// The zero-based position in the export's trigger input.
    pub trigger_ordinal: usize,
    /// The original route that first selected the output's media identity.
    pub variant_ordinal: usize,
    /// The variant providing the media, which may supply a full WEM for a prefetch.
    pub media_variant_ordinal: usize,
}

impl ExportedSound {
    /// Binds the measured byte length of a newly written file to the original WEM inputs.
    ///
    /// # Errors
    /// Returns an error when the caller's source evidence registry cannot retain a contribution.
    pub fn written_size_origin(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        self.size_origin.as_ref().map_or_else(
            || Ok(uses.origin(&[], false)),
            |size| size.bind(archive, uses),
        )
    }
}
