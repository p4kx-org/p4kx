// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_audio::Wem;

use crate::output_file::write_error;
use crate::{
    ConversionError, FieldOrigins, SourceEvidenceError, SourceInventorySelection, WavReport,
};

use super::origins::SoundOrigins;
use super::publication::SoundWrite;
use super::{Sound, SoundEncodeError, SoundOriginInput};

#[path = "sound_wav_origins.rs"]
mod origins;

#[path = "sound_wav_write.rs"]
mod written;
pub(crate) use written::WavWrite;

/// Summary and immediate source evidence for a published WAV and its companion.
#[non_exhaustive]
#[must_use]
pub struct WavFileReceipt {
    /// Report measured by the successful WAV writer.
    pub report: WavReport,
    /// Published sibling origin companion.
    pub companion: PathBuf,
    origins: SoundOrigins,
}

impl WavFileReceipt {
    /// Borrows the immediate origins of the written WAV summary fields.
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
    /// Writes PCM or ADPCM as WAV and publishes its bound source-origin companion.
    ///
    /// # Errors
    /// Returns the conversion failure or an artifact or companion publication failure.
    pub fn write_linear_wav_with_receipt(
        &self,
        out: &Path,
    ) -> Result<WavFileReceipt, ConversionError> {
        self.write_wav_receipt(out, |writer| WavWrite::linear(self.wem(), writer))
    }

    /// Writes a supported WAV and publishes its bound source-origin companion.
    ///
    /// # Errors
    /// Returns the conversion failure or an artifact or companion publication failure.
    pub fn write_wav_with_receipt(&self, out: &Path) -> Result<WavFileReceipt, ConversionError> {
        self.write_wav_receipt(out, |writer| WavWrite::encode(self.wem(), writer))
    }

    fn write_wav_receipt(
        &self,
        out: &Path,
        convert: impl FnOnce(&mut dyn Write) -> Result<WavWrite, SoundEncodeError>,
    ) -> Result<WavFileReceipt, ConversionError> {
        let publication = self.publish_sound(
            out,
            |writer, companion| {
                let written =
                    convert(writer).map_err(|cause| cause.into_conversion_error(self, out))?;
                let report = written.report();
                let origins = capture_wav(
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
        Ok(WavFileReceipt {
            report: publication.written.report,
            companion: publication.companion,
            origins: publication.written.origins,
        })
    }
}

pub(crate) fn capture_wav(
    wem: &Wem<impl AsRef<[u8]>>,
    input: Option<SoundOriginInput<'_>>,
    written: &WavWrite,
) -> io::Result<SoundOrigins> {
    origins::capture(wem, input, written)
}
