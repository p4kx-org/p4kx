// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use super::{MergedEvidence, Source, Step};
use crate::file_receipt::{self, StagedReceiptPublication, finish_staged_receipt};
use crate::output_file::StagedOutput;
use crate::textures::DdsMergeOrigins;
use crate::write::{EntryWriter, write_error};
use crate::{ExtractionError, FileReceipt, MergedTextureArtifact};

#[derive(Debug, Default)]
pub(crate) struct MergedOutput {
    pub(crate) step: Option<Step>,
    pub(crate) artifact: Option<MergedTextureArtifact>,
}

impl EntryWriter<'_> {
    pub(super) fn resolve_uncommitted_merge(
        &self,
        pending: Result<(), ExtractionError>,
        output: &mut MergedOutput,
    ) -> Result<(), ExtractionError> {
        match pending {
            Err(error)
                if self.tracker.should_stop()
                    && !crate::guard_cancellation::retained_output(&error) =>
            {
                output.step = Some(Step::Cancelled);
                Ok(())
            }
            other => other,
        }
    }

    pub(super) fn publish_merge(
        &self,
        source: &Source<'_>,
        staged: &StagedOutput<'_>,
        evidence: MergedEvidence,
        output: &mut MergedOutput,
    ) -> Result<(), ExtractionError> {
        let file = staged.output();
        let relative = file
            .strip_prefix(self.target)
            .map_err(|_| ExtractionError::UnsafePath)?;
        let companion = file_receipt::companion_path(relative)
            .map_err(|error| write_error(file.to_path_buf(), error))?;
        let companion_file = self.target.join(&companion);
        self.case_collision(&companion, source.header)
            .map_err(|error| write_error(companion_file.clone(), io::Error::other(error)))?;
        crate::portable_name::check_path_length(&companion_file)
            .map_err(|error| write_error(companion_file.clone(), io::Error::other(error)))?;
        let artifact = MergedTextureArtifact {
            path: report_path(relative)?,
            origin_receipt: report_path(&companion)?,
            digest: evidence.digest.clone(),
        };
        let receipt = FileReceipt::for_staged_replacement(
            file,
            self.protection,
            self.cancel.cloned().unwrap_or_default(),
        )
        .map_err(|error| write_error(companion_file.clone(), error))?;
        let origins = match evidence.files {
            Some(files) => DdsMergeOrigins::files(files, &evidence.spans),
            None => DdsMergeOrigins::archive(source.texture.parts(), &evidence.spans),
        }
        .map_err(|error| write_error(companion_file.clone(), error))?;
        let publication = receipt.publish_staged(
            staged,
            evidence.digest,
            || self.receipt_checkpoint(),
            |writer, binding| origins.write(writer, binding),
        );
        match publication {
            StagedReceiptPublication::Uncommitted(error) => {
                Err(write_error(file.to_path_buf(), error))
            }
            StagedReceiptPublication::Committed {
                receipt,
                retirement,
            } => {
                output.step = Some(Step::Written);
                let owner = source.header.index();
                self.tracker.written.record(relative, owner);
                if receipt.is_ok() {
                    self.tracker.written.record(&companion, owner);
                    output.artifact = Some(artifact);
                }
                finish_staged_receipt(receipt, retirement)
                    .map_err(|error| write_error(companion_file, error))
            }
        }
    }
}

fn report_path(relative: &Path) -> Result<String, ExtractionError> {
    relative
        .to_str()
        .map(|path| path.replace('\\', "/"))
        .ok_or(ExtractionError::UnsafePath)
}
