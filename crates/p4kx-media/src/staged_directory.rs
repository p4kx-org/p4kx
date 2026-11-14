// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::OwnedDirectoryPermit;

use crate::checkpoint::Checkpoint;
use crate::{MediaError, error};

pub(crate) struct StagedDirectory<'a> {
    permit: OwnedDirectoryPermit,
    checkpoint: Checkpoint<'a>,
    cleanup: bool,
}

impl<'a> StagedDirectory<'a> {
    pub(crate) fn create(
        parent: &Path,
        prefix: &str,
        checkpoint: Checkpoint<'a>,
    ) -> Result<Self, MediaError> {
        checkpoint.check()?;
        let carrier = tempfile::Builder::new()
            .prefix(prefix)
            .disable_cleanup(true)
            .make_in(parent, |path| {
                p4kx_extraction::create_output_directory(
                    path,
                    Some(checkpoint.protection),
                    checkpoint.cancel.flag(),
                )
                .map_err(p4kx_extraction::OwnedOutputError::into_io_error)
            })
            .map_err(|source| error::creation(parent, source))?;
        let (permit, _name) = carrier.into_parts();
        Ok(Self {
            permit,
            checkpoint,
            cleanup: true,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.permit.path()
    }

    pub(crate) const fn permit(&self) -> &OwnedDirectoryPermit {
        &self.permit
    }

    pub(crate) fn published_at(&mut self, path: &Path) -> Result<(), MediaError> {
        self.cleanup = false;
        self.permit
            .rebind_after_rename(path, self.checkpoint.cancel.flag())
            .map_err(|cause| MediaError::RetainedOutput {
                path: path.to_path_buf(),
                cause: Box::new(cause.into()),
            })
    }

    pub(crate) fn close(self) -> Result<(), MediaError> {
        self.finish(Ok(()))
    }

    pub(crate) fn finish<T>(mut self, result: Result<T, MediaError>) -> Result<T, MediaError> {
        let cleanup = if std::mem::replace(&mut self.cleanup, false) {
            self.permit
                .cleanup(self.checkpoint.cancel.flag())
                .map_err(MediaError::from)
        } else {
            Ok(())
        };
        error::finish(result, cleanup)
    }
}

impl Drop for StagedDirectory<'_> {
    fn drop(&mut self) {
        if self.cleanup {
            let _retained_on_failure = self.permit.cleanup(self.checkpoint.cancel.flag());
        }
    }
}
