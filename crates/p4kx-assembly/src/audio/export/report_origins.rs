// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, FieldOrigin, SourceEvidenceError, SourceUses};

use super::{ExportFailure, ExportLoopOrigins, ExportReport, ExportedSound};

/// Additional evidence requested from a sound export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportReportCapture {
    /// Retain the exact inputs to measured file sizes and the source positions of failures.
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

/// Additional evidence for an opted-in sound export report.
#[derive(Debug, Default)]
pub struct ExportReportOrigins {
    pub(super) loops: ExportLoopOrigins,
    failures: Vec<ExportFailureOrigin>,
}

impl ExportReportOrigins {
    /// Borrows the original WEM loop inputs retained for TXTP generation.
    #[must_use]
    pub const fn loop_origins(&self) -> &ExportLoopOrigins {
        &self.loops
    }

    /// Borrows the failure evidence in the report's exact error row order.
    #[must_use]
    pub const fn failures(&self) -> &[ExportFailureOrigin] {
        self.failures.as_slice()
    }

    pub(super) fn with_planned_failures(
        failures: Vec<FailedSound>,
        capture: bool,
    ) -> (ExportReport, Self) {
        let mut report = ExportReport::default();
        let mut origins = Self::default();
        report.failures.reserve(failures.len());
        if capture {
            origins.failures.reserve(failures.len());
        }
        for failure in failures {
            origins.push_failure(&mut report, failure, capture);
        }
        (report, origins)
    }

    pub(super) fn push_failure(
        &mut self,
        report: &mut ExportReport,
        failed: FailedSound,
        capture: bool,
    ) {
        report.failures.push(failed.failure);
        if capture {
            self.failures.push(failed.origin);
        }
    }
}

/// The original resolved inputs and operation for one reported failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportFailureOrigin {
    /// The zero-based trigger position in the export input.
    pub trigger_ordinal: usize,
    /// The zero-based variant position within that trigger.
    pub variant_ordinal: usize,
    /// The operation that admitted the failure into the report.
    pub kind: ExportFailureKind,
}

/// The operation that reported a sound export failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFailureKind {
    /// The authored source has no media file.
    FilelessSource,
    /// The planner could not select a media location.
    UnresolvedMedia,
    /// A planned job could not complete.
    Job,
}

#[derive(Debug)]
pub(super) struct FailedSound {
    failure: ExportFailure,
    origin: ExportFailureOrigin,
}

impl FailedSound {
    pub(super) fn new(
        trigger: &str,
        wem_id: Option<u32>,
        reason: String,
        origin: ExportFailureOrigin,
    ) -> Self {
        Self {
            failure: ExportFailure {
                trigger: trigger.to_owned(),
                wem_id,
                reason,
            },
            origin,
        }
    }
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
