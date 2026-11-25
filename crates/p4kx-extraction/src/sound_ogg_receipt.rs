// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_audio::Wem;

use crate::output_file::write_error;
use crate::{
    ConversionError, FieldOrigins, OggReport, SourceEvidenceError, SourceInventorySelection,
};

use super::origins::SoundOrigins;
use super::publication::SoundWrite;
use super::{Sound, SoundOriginInput};

#[path = "sound_ogg_origins.rs"]
mod origins;

#[path = "sound_ogg_write.rs"]
mod written;
pub(crate) use written::OggWrite;

/// Summary and immediate source evidence for a published Ogg stream and its companion.
#[non_exhaustive]
#[must_use]
pub struct OggFileReceipt {
    /// Report measured by the successful Ogg writer.
    pub report: OggReport,
    /// Published sibling origin companion.
    pub companion: PathBuf,
    origins: SoundOrigins,
}

impl OggFileReceipt {
    /// Borrows the immediate origins of the written Ogg summary fields.
    #[must_use]
    pub const fn summary_origins(&self) -> &FieldOrigins {
        self.origins.summary_origins()
    }

    /// Resolves the physical inputs of the summary's immediate field origins.
    ///
    /// # Errors
    /// Returns an error if a summary reference cannot be resolved in the captured inventory.
    pub fn summary_provenance(&self) -> Result<SourceInventorySelection<'_>, SourceEvidenceError> {
        self.origins.summary_provenance()
    }
}

impl Sound {
    /// Writes Vorbis or Opus as Ogg and publishes its bound source-origin companion.
    ///
    /// # Errors
    /// Returns the conversion failure or an artifact or companion publication failure.
    pub fn write_ogg_with_receipt(&self, out: &Path) -> Result<OggFileReceipt, ConversionError> {
        let publication = self.publish_sound(
            out,
            |writer, companion| {
                let written = OggWrite::encode(self.wem(), writer)
                    .map_err(|cause| cause.into_conversion_error(self, out))?;
                let report = written.report();
                let origins = capture_ogg(
                    self.wem(),
                    self.source_input().map(SoundOriginInput::loose),
                    &written,
                )
                .map_err(|source| write_error(companion, source))?;
                Ok(SoundWrite {
                    report,
                    byte_length: report.size,
                    origins,
                })
            },
            SoundOrigins::write,
        )?;
        Ok(OggFileReceipt {
            report: publication.written.report,
            companion: publication.companion,
            origins: publication.written.origins,
        })
    }
}

pub(crate) fn capture_ogg(
    wem: &Wem<impl AsRef<[u8]>>,
    input: Option<SoundOriginInput<'_>>,
    written: &OggWrite,
) -> io::Result<SoundOrigins> {
    origins::capture(wem, input, written)
}
