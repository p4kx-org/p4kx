// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::write::write_error;
use crate::{ArchiveWriteProtection, CancelToken, ExtractionError};

const MAX_TEMPORARY_ATTEMPTS: u32 = 100;
static TEMPORARY_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) fn create_folders_protected(
    target: &Path,
    relative: Option<&Path>,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
) -> Result<PathBuf, ExtractionError> {
    let mut folder = target.to_path_buf();
    for component in relative.into_iter().flat_map(Path::components) {
        folder.push(component);
        crate::check_protected_output(&folder, protection, cancel)?;
        match fs::create_dir(&folder) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&folder)
                    .map_err(|source| write_error(folder.clone(), source))?;
                if metadata.file_type().is_symlink() {
                    return Err(ExtractionError::UnsafePath);
                }
                if !metadata.is_dir() {
                    return Err(write_error(folder, error));
                }
            }
            Err(source) => return Err(write_error(folder, source)),
        }
    }
    Ok(folder)
}

pub(crate) fn create_temporary_protected(
    folder: &Path,
    protection: Option<&ArchiveWriteProtection>,
    cancel: Option<&CancelToken>,
) -> Result<(PathBuf, File, crate::OwnedFileToken), ExtractionError> {
    let mut last_error = None;
    for _ in 0..MAX_TEMPORARY_ATTEMPTS {
        let number = TEMPORARY_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = folder.join(format!(".p4kx-{}-{number}.part", process::id()));
        crate::check_protected_output(&path, protection, cancel)?;
        match crate::create_output_file(&path, protection, crate::output_file::cancel_flag(cancel))
        {
            Ok(created) => {
                let (file, token) = created.into_parts();
                return Ok((path, file, token));
            }
            Err(crate::OwnedOutputError::Io { source, .. })
                if source.kind() == io::ErrorKind::AlreadyExists =>
            {
                last_error = Some(source);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(write_error(
        folder.to_path_buf(),
        last_error.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists)),
    ))
}
