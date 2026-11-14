// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::OwnedFileToken;
use tempfile::NamedTempFile;

use crate::checkpoint::Checkpoint;
use crate::publication::Publication;
use crate::{MediaError, error};

struct FileState {
    file: File,
    token: OwnedFileToken,
}

pub(crate) struct TemporaryFile<'a> {
    file: Option<NamedTempFile<FileState>>,
    checkpoint: Checkpoint<'a>,
    cleanup: bool,
}

impl<'a> TemporaryFile<'a> {
    pub(crate) fn create(parent: &Path, checkpoint: Checkpoint<'a>) -> Result<Self, MediaError> {
        checkpoint.check()?;
        let file = tempfile::Builder::new()
            .disable_cleanup(true)
            .make_in(parent, |path| {
                let created = p4kx_extraction::create_output_file(
                    path,
                    Some(checkpoint.protection),
                    checkpoint.cancel.flag(),
                )
                .map_err(p4kx_extraction::OwnedOutputError::into_io_error)?;
                let (file, token) = created.into_parts();
                Ok(FileState { file, token })
            })
            .map_err(|source| error::creation(parent, source))?;
        Ok(Self {
            file: Some(file),
            checkpoint,
            cleanup: true,
        })
    }

    fn file(&self) -> Result<&NamedTempFile<FileState>, MediaError> {
        self.file
            .as_ref()
            .ok_or_else(|| MediaError::Contract("temporary file already placed".into()))
    }

    pub(crate) fn ensure_writable(&self) -> Result<(), MediaError> {
        let file = self.file()?;
        file.as_file()
            .token
            .ensure_writable(file.path(), self.checkpoint.cancel.flag())?;
        Ok(())
    }

    pub(crate) fn sync_all(&self) -> Result<(), MediaError> {
        self.ensure_writable()?;
        let file = self.file()?;
        file.as_file()
            .file
            .sync_all()
            .map_err(|source| error::io(file.path(), source))
    }

    pub(crate) fn persist(
        &mut self,
        target: &Path,
        publication: Publication<'_>,
    ) -> Result<(), MediaError> {
        let temporary = self.file()?;
        temporary
            .as_file()
            .token
            .ensure_writable(temporary.path(), self.checkpoint.cancel.flag())?;
        publication.check(target, &self.checkpoint)?;
        let file = self
            .file
            .take()
            .ok_or_else(|| MediaError::Contract("temporary file already placed".into()))?;
        match file.persist(target) {
            Ok(mut state) => {
                state
                    .token
                    .rebind_after_rename(target, self.checkpoint.cancel.flag())
                    .map_err(|cause| MediaError::RetainedOutput {
                        path: target.to_path_buf(),
                        cause: Box::new(cause.into()),
                    })?;
                Ok(())
            }
            Err(failure) => {
                self.file = Some(failure.file);
                Err(error::io(target, failure.error))
            }
        }
    }

    pub(crate) fn finish<T>(mut self, result: Result<T, MediaError>) -> Result<T, MediaError> {
        self.cleanup = false;
        let cleanup = self.file.as_ref().map_or(Ok(()), |file| {
            file.as_file()
                .token
                .remove(file.path(), self.checkpoint.cancel.flag())
                .map_err(MediaError::from)
        });
        error::finish(result, cleanup)
    }
}

impl Write for TemporaryFile<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("temporary file already placed"))?
            .as_file_mut()
            .file
            .write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("temporary file already placed"))?
            .as_file_mut()
            .file
            .flush()
    }
}

impl Drop for TemporaryFile<'_> {
    fn drop(&mut self) {
        if self.cleanup
            && let Some(file) = &self.file
        {
            let _retained_on_failure = file
                .as_file()
                .token
                .remove(file.path(), self.checkpoint.cancel.flag());
        }
    }
}
