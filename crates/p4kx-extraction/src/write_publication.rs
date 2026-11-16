// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::{Chunk, EntryWriter, Outcome, write_error};
use crate::file_receipt::OrdinaryReplacement;
use crate::output_file::StagedOutput;
use crate::{ExtractionError, Texture};

impl EntryWriter<'_> {
    /// Renames `temporary` to `destination` once `copied` reached the end of the data, or removes
    /// it; returns `done` once the file is in place.
    pub(crate) fn put_in_place(
        &self,
        temporary: &Path,
        ownership: &crate::OwnedFileToken,
        destination: &Path,
        copied: Result<Chunk, ExtractionError>,
        done: Outcome,
    ) -> Result<Outcome, ExtractionError> {
        if !matches!(copied, Ok(Chunk::End)) || self.tracker.archive_file_changed() {
            return crate::output_cleanup::extraction(
                copied.map(|_| Outcome::Cancelled),
                ownership,
                temporary,
                self.cancel,
            );
        }
        let staged = StagedOutput::new(
            temporary,
            destination,
            ownership,
            self.protection,
            self.cancel,
        );
        let mut committed = false;
        let replace = || {
            staged.check()?;
            fs::rename(temporary, destination)
                .map_err(|source| write_error(destination.to_path_buf(), source))?;
            committed = true;
            Ok(done)
        };
        let result = if destination
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(Texture::is_header_path)
        {
            self.replace_dds_without_receipt(&staged, replace)
        } else {
            replace()
        };
        if committed {
            result
        } else {
            crate::output_cleanup::extraction(result, ownership, temporary, self.cancel)
        }
    }

    fn replace_dds_without_receipt(
        &self,
        staged: &StagedOutput<'_>,
        replace: impl FnOnce() -> Result<Outcome, ExtractionError>,
    ) -> Result<Outcome, ExtractionError> {
        staged.check()?;
        let output = staged.output();
        let replacement = OrdinaryReplacement::new(output, self.protection, self.cancel)
            .map_err(|error| write_error(output.to_path_buf(), error))?;
        let result = (|| {
            staged.check()?;
            if self.tracker.should_stop() {
                return Ok(Outcome::Cancelled);
            }
            replacement
                .invalidate()
                .map_err(|error| write_error(output.to_path_buf(), error))?;
            if self.tracker.archive_file_changed() {
                return Ok(Outcome::Cancelled);
            }
            replace()
        })();
        replacement.finish_extraction(output, result)
    }
}
