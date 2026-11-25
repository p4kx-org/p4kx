// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::Path;

use super::{Chunk, EntryWriter, Outcome, read_existing_error, write_error};
use crate::file_receipt::{
    FreshPublication, OrdinaryReplacement, origin_lock_collision, try_create_fresh,
};
use crate::origin_file_io;
use crate::output_file::StagedOutput;
use crate::{ConflictPolicy, ExtractionError};

impl EntryWriter<'_> {
    /// Refuses publication after the extraction or its Archive watch stops.
    pub(crate) fn receipt_checkpoint(&self) -> io::Result<()> {
        if self.tracker.should_stop() {
            return Err(io::ErrorKind::Interrupted.into());
        }
        Ok(())
    }

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
        let cleanup =
            |result| crate::output_cleanup::extraction(result, ownership, temporary, self.cancel);
        match try_create_fresh(&staged, self.cancel, || self.tracker.should_stop()) {
            Ok(FreshPublication::Installed) => return cleanup(Ok(done)),
            Ok(FreshPublication::Cancelled) => return cleanup(Ok(Outcome::Cancelled)),
            Ok(FreshPublication::Contended) if self.policy == ConflictPolicy::Skip => {
                return cleanup(Ok(Outcome::Kept));
            }
            Ok(FreshPublication::Contended | FreshPublication::RequiresLock) => {}
            Err(error) => return cleanup(Err(error)),
        }
        let mut moved = false;
        let replace = || {
            staged.check()?;
            if self.policy == ConflictPolicy::Skip {
                return install_without_clobbering(&staged, done);
            }
            fs::rename(temporary, destination)
                .map_err(|source| write_error(destination.to_path_buf(), source))?;
            moved = true;
            Ok(done)
        };
        let result = self.replace_without_receipt(&staged, replace);
        if moved { result } else { cleanup(result) }
    }

    fn replace_without_receipt(
        &self,
        staged: &StagedOutput<'_>,
        replace: impl FnOnce() -> Result<Outcome, ExtractionError>,
    ) -> Result<Outcome, ExtractionError> {
        staged.check()?;
        let output = staged.output();
        if self.keep_existing(output)? {
            return Ok(Outcome::Kept);
        }
        let replacement = OrdinaryReplacement::for_entry(output, self.protection, self.cancel)
            .map_err(|error| replacement_error(output, error))?;
        let result = (|| {
            staged.check()?;
            if self.tracker.should_stop() {
                return Ok(Outcome::Cancelled);
            }
            if self.keep_existing(output)? {
                return Ok(Outcome::Kept);
            }
            replacement
                .invalidate()
                .map_err(|error| replacement_error(output, error))?;
            if self.tracker.archive_file_changed() {
                return Ok(Outcome::Cancelled);
            }
            replace()
        })();
        replacement.finish_extraction(output, result)
    }

    fn keep_existing(&self, output: &Path) -> Result<bool, ExtractionError> {
        if self.policy != ConflictPolicy::Skip {
            return Ok(false);
        }
        match fs::symlink_metadata(output) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(read_existing_error(output, error)),
        }
    }
}

fn install_without_clobbering(
    staged: &StagedOutput<'_>,
    done: Outcome,
) -> Result<Outcome, ExtractionError> {
    let output = staged.output();
    let installed = origin_file_io::install_without_replacing(staged, output)
        .map_err(|error| write_error(output.to_path_buf(), error))?;
    Ok(if installed { done } else { Outcome::Kept })
}

fn replacement_error(output: &Path, error: io::Error) -> ExtractionError {
    if let Some(path) = origin_lock_collision(&error) {
        return ExtractionError::OriginLockCollision {
            path: path.to_path_buf(),
        };
    }
    write_error(output.to_path_buf(), error)
}

#[cfg(test)]
#[path = "write_publication_tests.rs"]
mod tests;
