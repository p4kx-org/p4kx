// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use crate::file_receipt::{self, OrdinaryReplacement};
use crate::output_file::{self, write_error};
use crate::{
    ConversionError, FieldOrigins, OriginDigestWriter, SourceEvidenceError,
    SourceInventorySelection, WavReport,
};

use super::Sound;

#[path = "sound_wav_origins.rs"]
mod origins;
use origins::WavOrigins;

/// Summary and immediate source evidence for a published WAV and its companion.
#[non_exhaustive]
#[must_use]
pub struct WavFileReceipt {
    /// Report measured by the successful WAV writer.
    pub report: WavReport,
    /// Published sibling origin companion.
    pub companion: PathBuf,
    origins: WavOrigins,
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
        self.refuse_input_as_output(out)?;
        let companion =
            file_receipt::companion_path(out).map_err(|source| write_error(out, source))?;
        self.refuse_input_as_output(&companion)?;
        let protection = self.protection.context()?;
        let mut held = None;
        let written = output_file::write_replacing_protected_before(
            out,
            protection,
            None,
            |writer| {
                let mut measured = OriginDigestWriter::new(writer);
                let result = self
                    .wem
                    .write_linear_wav(&mut measured)
                    .map_err(|source| self.audio_error(out, source))?;
                let digest = measured
                    .finish()
                    .map_err(|source| write_error(out, source))?;
                if digest.byte_length() != result.report.size {
                    return Err(write_error(
                        out,
                        io::Error::other("WAV size differs from its write report"),
                    ));
                }
                let origins = WavOrigins::capture(self, result)
                    .map_err(|source| write_error(&companion, source))?;
                Ok((result.report, digest, origins))
            },
            |staged| {
                let replacement = OrdinaryReplacement::new(out, protection, None)
                    .map_err(|source| write_error(out, source))?;
                let replacement = held.insert(replacement);
                staged.check()?;
                replacement
                    .invalidate_for_companion()
                    .map_err(|source| write_error(&companion, source))
            },
        );
        let replacement = held.ok_or_else(|| {
            write_error(
                out,
                io::Error::other("WAV committed without a receipt lock"),
            )
        });
        match (written, replacement) {
            (Ok(((report, digest, origins), ())), Ok(replacement)) => {
                replacement.finish_companion(out, (), digest, |writer, binding| {
                    origins.write(writer, binding)
                })?;
                Ok(WavFileReceipt {
                    report,
                    companion,
                    origins,
                })
            }
            (Err(error), Ok(replacement)) => replacement.finish_conversion(out, Err(error)),
            (Err(error), Err(_)) | (Ok(_), Err(error)) => Err(error),
        }
    }
}
