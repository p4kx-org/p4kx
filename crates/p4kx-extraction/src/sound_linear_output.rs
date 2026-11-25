// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_audio::{AudioError, LinearWavWrite, Wem};

use super::{LooseSound, SoundOutput};
use crate::file_receipt::{self, StagedReceiptPublication, finish_staged_receipt};
use crate::output_file::StagedOutput;
use crate::sound::capture_linear_wav;
use crate::texture_convert::Step;
use crate::write::{Destination, EntryWriter, write_error};
use crate::{
    ConflictPolicy, ConvertedSoundArtifact, ExtractionError, FileReceipt, OriginArtifactDigest,
    OriginDigestWriter,
};

impl EntryWriter<'_> {
    pub(crate) fn write_linear_sound(
        &self,
        destination: &Destination,
        wem: &Wem<&[u8]>,
        source: &LooseSound<'_>,
        chunk: &mut [u8],
    ) -> Result<Result<SoundOutput, AudioError>, ExtractionError> {
        self.stage_converted(
            destination,
            |sink| {
                let mut measured = OriginDigestWriter::new(sink);
                let written = wem.write_linear_wav(&mut measured)?;
                let digest = measured.finish().map_err(AudioError::Write)?;
                if digest.byte_length() != written.report.size {
                    return Err(AudioError::Write(io::Error::other(
                        "WAV size differs from its write report",
                    )));
                }
                Ok(LinearOutput { digest, written })
            },
            |temporary, ownership, output| {
                let staged = StagedOutput::new(
                    temporary,
                    &destination.file,
                    ownership,
                    self.protection,
                    self.cancel,
                );
                let result = self.publish_linear_sound(&staged, source, wem, output);
                if self.policy != ConflictPolicy::Skip
                    && result
                        .as_ref()
                        .is_ok_and(|placed| placed.step == Step::Written)
                {
                    return result;
                }
                let result = match result {
                    Err(error)
                        if self.tracker.should_stop()
                            && !crate::guard_cancellation::retained_output(&error) =>
                    {
                        Ok(SoundOutput::of(Step::Cancelled))
                    }
                    result => result,
                };
                crate::output_cleanup::extraction(result, ownership, temporary, self.cancel)
            },
            chunk,
        )
    }

    fn publish_linear_sound(
        &self,
        staged: &StagedOutput<'_>,
        source: &LooseSound<'_>,
        wem: &Wem<&[u8]>,
        output: LinearOutput,
    ) -> Result<SoundOutput, ExtractionError> {
        let artifact = staged.output();
        let relative = artifact
            .strip_prefix(self.target)
            .map_err(|_| ExtractionError::UnsafePath)?;
        let companion = file_receipt::companion_path(relative)
            .map_err(|error| write_error(artifact.to_path_buf(), error))?;
        let receipt_path = self.target.join(&companion);
        self.case_collision(relative, &source.entry)?;
        self.case_collision(&companion, &source.entry)?;
        crate::portable_name::check_path_length(&receipt_path)?;

        let reported = ConvertedSoundArtifact {
            path: relative_path(relative)?,
            origin_receipt: relative_path(&companion)?,
            digest: output.digest.clone(),
        };
        let origins = capture_linear_wav(wem, source.input, output.written)
            .map_err(|error| write_error(receipt_path.clone(), error))?;
        let receipt = FileReceipt::for_staged_replacement(
            artifact,
            self.protection,
            self.cancel.cloned().unwrap_or_default(),
        )
        .map_err(|error| write_error(receipt_path.clone(), error))?;
        match receipt.publish_staged_with_policy(
            staged,
            output.digest,
            self.policy,
            || self.receipt_checkpoint(),
            |writer, binding| origins.write(writer, binding),
        ) {
            Some(StagedReceiptPublication::Uncommitted(error)) => {
                Err(write_error(artifact.to_path_buf(), error))
            }
            None => Ok(SoundOutput::of(Step::Kept)),
            Some(StagedReceiptPublication::Committed {
                receipt,
                retirement,
            }) => {
                let owner = source.entry.index();
                self.tracker.written.record(relative, owner);
                let artifact = if receipt.is_ok() {
                    self.tracker.written.record(&companion, owner);
                    Some(reported)
                } else {
                    None
                };
                let receipt_failure = finish_staged_receipt(receipt, retirement)
                    .map_err(|error| write_error(receipt_path, error))
                    .err();
                Ok(SoundOutput {
                    step: Step::Written,
                    artifact,
                    receipt_failure,
                })
            }
        }
    }
}

struct LinearOutput {
    digest: OriginArtifactDigest,
    written: LinearWavWrite,
}

fn relative_path(path: &Path) -> Result<String, ExtractionError> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or(ExtractionError::UnsafePath)
}
