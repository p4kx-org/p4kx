// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::file_receipt::{self, OrdinaryReplacement};
use crate::output_file::{self, write_error};
use crate::{ConversionError, OriginArtifactBinding, OriginDigestWriter};

use super::Sound;

pub(super) struct SoundWrite<Report, Origins> {
    pub(super) report: Report,
    pub(super) byte_length: u64,
    pub(super) origins: Origins,
}

pub(super) struct SoundPublication<Report, Origins> {
    pub(super) written: SoundWrite<Report, Origins>,
    pub(super) companion: PathBuf,
}

impl Sound {
    pub(super) fn publish_sound<Report, Origins>(
        &self,
        out: &Path,
        convert: impl FnOnce(
            &mut dyn Write,
            &Path,
        ) -> Result<SoundWrite<Report, Origins>, ConversionError>,
        describe: impl FnOnce(&Origins, &mut dyn Write, &OriginArtifactBinding) -> io::Result<()>,
    ) -> Result<SoundPublication<Report, Origins>, ConversionError> {
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
                let written = convert(&mut measured, &companion)?;
                let digest = measured
                    .finish()
                    .map_err(|source| write_error(out, source))?;
                if digest.byte_length() != written.byte_length {
                    return Err(write_error(
                        out,
                        io::Error::other("Audio size differs from its write report"),
                    ));
                }
                Ok((written, digest))
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
                io::Error::other("Audio committed without a receipt lock"),
            )
        });
        match (written, replacement) {
            (Ok(((written, digest), (), owner)), Ok(replacement)) => {
                replacement.finish_companion(out, &owner, digest, |writer, binding| {
                    describe(&written.origins, writer, binding)
                })?;
                Ok(SoundPublication { written, companion })
            }
            (Err(error), Ok(replacement)) => replacement.finish_conversion(out, Err(error)),
            (Err(error), Err(_)) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

#[cfg(test)]
#[path = "sound_publication_tests.rs"]
mod tests;
