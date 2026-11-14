// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_extraction::{
    CancelToken, ExtractionError, OwnedFileToken, OwnedOutputError, RecordExportOptions,
    check_protected_output, create_output_file,
};
use tempfile::TempPath;

const WRITE_BUFFER_LEN: usize = 64 << 10;

/// The active descriptor and creation proof for one streamed Record file.
pub(crate) struct Output {
    file: Option<BufWriter<File>>,
    temporary: Option<TempPath>,
    ownership: OwnedFileToken,
    pub(crate) len: u64,
    pub(crate) crc: u32,
}

impl Output {
    pub(crate) fn create(
        target: &Path,
        options: &RecordExportOptions<'_>,
    ) -> Result<Self, ExtractionError> {
        let file = tempfile::Builder::new()
            .prefix(".p4kx-")
            .suffix(".part")
            .disable_cleanup(true)
            .make_in(target, |path| {
                create_output_file(path, options.protection, cancel_flag(options.cancel))
                    .map_err(OwnedOutputError::into_io_error)
            })
            .map_err(|source| {
                if let Some(OwnedOutputError::Protection(error)) =
                    p4kx_extraction::owned_output_failure(&source)
                {
                    return ExtractionError::Installation(error.clone());
                }
                write_error(target, source)
            })?;
        let (created, temporary) = file.into_parts();
        let (file, ownership) = created.into_parts();
        Ok(Self {
            file: Some(BufWriter::with_capacity(WRITE_BUFFER_LEN, file)),
            temporary: Some(temporary),
            ownership,
            len: 0,
            crc: 0,
        })
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("Record output is closed"))?
            .write_all(bytes)?;
        self.len = self.len.saturating_add(bytes.len() as u64);
        self.crc = crc32c::crc32c_append(self.crc, bytes);
        Ok(())
    }

    pub(crate) fn persist(
        &mut self,
        destination: &Path,
        replace: bool,
        options: &RecordExportOptions<'_>,
    ) -> Result<bool, ExtractionError> {
        if let Some(writer) = self.file.take() {
            writer
                .into_inner()
                .map_err(|error| write_error(destination, error.into_error()))?
                .sync_all()
                .map_err(|error| write_error(destination, error))?;
        }
        let path = self.temporary.as_ref().ok_or_else(|| {
            write_error(destination, io::Error::other("Record temporary is closed"))
        })?;
        self.ownership
            .ensure_writable(path, cancel_flag(options.cancel))?;
        check_protected_output(destination, options.protection, options.cancel)?;
        let temporary = self.temporary.take().ok_or_else(|| {
            write_error(destination, io::Error::other("Record temporary is closed"))
        })?;
        let persisted = if replace {
            temporary.persist(destination)
        } else {
            temporary.persist_noclobber(destination)
        };
        match persisted {
            Ok(()) => Ok(false),
            Err(error) => {
                self.temporary = Some(error.path);
                if !replace && error.error.kind() == io::ErrorKind::AlreadyExists {
                    Ok(true)
                } else {
                    Err(write_error(destination, error.error))
                }
            }
        }
    }

    pub(crate) fn complete<T>(
        mut self,
        result: Result<T, ExtractionError>,
        cancel: Option<&CancelToken>,
    ) -> Result<T, ExtractionError> {
        self.file = None;
        let Some(temporary) = self.temporary else {
            return result;
        };
        match self.ownership.remove(&temporary, cancel_flag(cancel)) {
            Ok(()) => result,
            Err(cleanup) => Err(ExtractionError::CleanupFailed {
                primary: result
                    .err()
                    .map(|error| Box::new(error) as Box<dyn std::error::Error + Send + Sync>),
                cleanup: Box::new(cleanup),
            }),
        }
    }
}

fn cancel_flag(cancel: Option<&CancelToken>) -> &std::sync::atomic::AtomicBool {
    static NEVER_CANCELLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    cancel.map_or(&NEVER_CANCELLED, CancelToken::flag)
}

fn write_error(path: &Path, source: io::Error) -> ExtractionError {
    ExtractionError::Write {
        path: path.to_path_buf(),
        source,
    }
}
