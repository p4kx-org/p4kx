// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

use p4kx_extraction::{
    ArchiveWriteProtection, CancelToken, OwnedFileToken, OwnedOutputCleanupError, OwnedOutputError,
    check_protected_output, create_output_file,
};

/// A catalog file written next to its target and moved over it on [`commit`](Self::commit).
///
/// The target stays unchanged until commit; identity failures retain untrusted temporaries.
#[derive(Debug)]
pub struct CatalogFile<'a> {
    protection: Option<&'a ArchiveWriteProtection>,
    cancel: Option<&'a CancelToken>,
    file: Option<File>,
    ownership: OwnedFileToken,
    temporary: PathBuf,
    out: PathBuf,
}

impl<'a> CatalogFile<'a> {
    /// Creates the temporary file next to `out`, open for writing.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when `out` names no file or the temporary file cannot be created.
    pub fn create(out: &Path) -> io::Result<Self> {
        Self::create_checked(out, None, None)
    }

    /// Creates a catalog file under the immutable source protection of one Archive job.
    ///
    /// # Errors
    /// Returns a typed protection refusal inside the I/O error or a file creation failure.
    pub fn create_protected(
        out: &Path,
        protection: &'a ArchiveWriteProtection,
        cancel: &'a CancelToken,
    ) -> io::Result<Self> {
        Self::create_checked(out, Some(protection), Some(cancel))
    }

    fn create_checked(
        out: &Path,
        protection: Option<&'a ArchiveWriteProtection>,
        cancel: Option<&'a CancelToken>,
    ) -> io::Result<Self> {
        check_output(out, protection, cancel)?;
        let name = out.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} does not name a file", out.display()),
            )
        })?;
        let mut name = name.to_os_string();
        name.push(format!(".p4kx-{}.part", process::id()));
        let temporary = out.with_file_name(name);
        check_output(&temporary, protection, cancel)?;
        let (file, ownership) = create_output_file(&temporary, protection, cancel_flag(cancel))
            .map_err(OwnedOutputError::into_io_error)?
            .into_parts();
        Ok(Self {
            protection,
            cancel,
            file: Some(file),
            ownership,
            temporary,
            out: out.to_path_buf(),
        })
    }

    /// Flushes the file to disk and replaces the target with it.
    ///
    /// # Errors
    ///
    /// Returns the I/O error when the file cannot be flushed or cannot replace the target; the
    /// temporary file is removed then.
    pub fn commit(mut self) -> io::Result<()> {
        let result = (|| {
            if let Some(file) = self.file.take() {
                file.sync_all()?;
            }
            self.ownership
                .ensure_writable(&self.temporary, cancel_flag(self.cancel))
                .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))?;
            check_output(&self.out, self.protection, self.cancel)?;
            fs::rename(&self.temporary, &self.out)?;
            self.temporary = PathBuf::new();
            Ok(())
        })();
        match result {
            Ok(()) => Ok(()),
            Err(source) => {
                let path = self.out.clone();
                match self.discard() {
                    Ok(()) => Err(source),
                    Err(cleanup) => Err(OwnedOutputError::Cleanup {
                        primary: Box::new(OwnedOutputError::Io { path, source }),
                        cleanup,
                    }
                    .into_io_error()),
                }
            }
        }
    }

    /// Removes the unpublished created file using its retained descriptor identity.
    ///
    /// # Errors
    /// Returns an identity or unlink failure and retains an unauthenticated temporary.
    pub fn discard(mut self) -> Result<(), OwnedOutputCleanupError> {
        self.file = None;
        self.ownership
            .remove(&self.temporary, cancel_flag(self.cancel))?;
        self.temporary = PathBuf::new();
        Ok(())
    }

    fn open(&mut self) -> io::Result<&mut File> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("the catalog file is closed"))
    }
}

impl Write for CatalogFile<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.open()?.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.open()?.flush()
    }
}

impl Drop for CatalogFile<'_> {
    fn drop(&mut self) {
        self.file = None;
        if !self.temporary.as_os_str().is_empty() {
            let _cleanup = self
                .ownership
                .remove(&self.temporary, cancel_flag(self.cancel));
        }
    }
}

fn cancel_flag(cancel: Option<&CancelToken>) -> &std::sync::atomic::AtomicBool {
    static NEVER_CANCELLED: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
    cancel.map_or(&NEVER_CANCELLED, CancelToken::flag)
}

fn check_output(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
) -> io::Result<()> {
    check_protected_output(path, protection, cancel)
        .map_err(|error| io::Error::new(io::ErrorKind::PermissionDenied, error))
}
