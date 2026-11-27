// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use crate::{ArchiveWriteProtection, OwnedFileToken, OwnedOutputError};

/// One exclusively created file and its descriptor-bound ownership evidence.
#[derive(Debug)]
pub struct CreatedOutputFile {
    file: File,
    token: OwnedFileToken,
}

impl CreatedOutputFile {
    /// Transfers the file and its ownership token to a streaming writer.
    #[must_use]
    pub fn into_parts(self) -> (File, OwnedFileToken) {
        (self.file, self.token)
    }
}

/// Creates a new file after source and target protection, retaining its actual file identity.
///
/// # Errors
/// Returns a protection refusal or the original exclusive-creation failure.
pub fn create_output_file(
    path: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: &AtomicBool,
) -> Result<CreatedOutputFile, OwnedOutputError> {
    create_with(
        path,
        protection,
        cancel,
        OwnedFileToken::capture,
        OwnedFileToken::writer,
    )
}

fn create_with<'protection>(
    path: &Path,
    protection: Option<&'protection ArchiveWriteProtection>,
    cancel: &AtomicBool,
    capture: impl FnOnce(
        &Path,
        PathBuf,
        File,
        Option<&'protection ArchiveWriteProtection>,
    ) -> Result<OwnedFileToken, OwnedOutputError>,
    writer: impl FnOnce(&OwnedFileToken) -> io::Result<File>,
) -> Result<CreatedOutputFile, OwnedOutputError> {
    if let Some(protection) = protection {
        protection.ensure_writable(path, cancel)?;
    } else {
        crate::write_target::host_target(path, cancel)?;
        crate::ensure_writable(path)?;
    }
    let host = crate::write_target::host_target(path, cancel)?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| OwnedOutputError::io(path, source))?;
    let token = capture(path, host, file, protection)?;
    if let Err(primary) = token.ensure_writable(path, cancel) {
        let primary = OwnedOutputError::Protection(primary);
        return Err(failed(&token, path, cancel, primary));
    }
    let file = writer(&token)
        .map_err(|source| failed(&token, path, cancel, OwnedOutputError::io(path, source)))?;
    Ok(CreatedOutputFile { file, token })
}

fn failed(
    token: &OwnedFileToken,
    path: &Path,
    cancel: &AtomicBool,
    primary: OwnedOutputError,
) -> OwnedOutputError {
    match token.remove(path, cancel) {
        Ok(()) => primary,
        Err(cleanup) => OwnedOutputError::Cleanup {
            primary: Box::new(primary),
            cleanup,
        },
    }
}

#[cfg(test)]
mod tests;
