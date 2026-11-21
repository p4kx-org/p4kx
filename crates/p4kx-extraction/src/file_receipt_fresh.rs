// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Component, Path};

use crate::origin_file_io;
use crate::output_file::{StagedOutput, cancel_flag};
use crate::write::write_error;
use crate::{CancelToken, ExtractionError, is_redirected_metadata, validated_output_directory};

use super::DIRECTORY_NAME;
use super::lock::is_origin_lock_name;
use super::target_path::{is_companion_name, sibling_receipt};

#[cfg(test)]
#[path = "file_receipt_fresh_tests.rs"]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FreshPublication {
    Installed,
    Contended,
    RequiresLock,
    Cancelled,
}

pub(crate) fn try_create_fresh(
    staged: &StagedOutput<'_>,
    cancel: Option<&CancelToken>,
    stopped: impl FnOnce() -> bool,
) -> Result<FreshPublication, ExtractionError> {
    try_create_with(
        staged,
        cancel,
        stopped,
        origin_file_io::install_without_replacing,
    )
}

fn try_create_with(
    staged: &StagedOutput<'_>,
    cancel: Option<&CancelToken>,
    stopped: impl FnOnce() -> bool,
    install: impl FnOnce(&StagedOutput<'_>, &Path) -> io::Result<bool>,
) -> Result<FreshPublication, ExtractionError> {
    let output = staged.output();
    let (Some(parent), Some(name)) = (output.parent(), output.file_name()) else {
        return Ok(FreshPublication::RequiresLock);
    };
    if !output.is_absolute()
        || staged.temporary().parent() != Some(parent)
        || parent.components().any(|part| part == Component::ParentDir)
        || is_origin_lock_name(name)
        || is_companion_name(name)
        || !is_absent(output)
    {
        return Ok(FreshPublication::RequiresLock);
    }
    let mut no_prior_proof = false;
    let flag = cancel_flag(cancel);
    let permit = validated_output_directory(parent, None, flag, |bound| {
        let sibling = sibling_receipt(&bound.join(name))?;
        no_prior_proof = is_absent(&sibling) && is_absent(&bound.join(DIRECTORY_NAME));
        Ok(())
    });
    let permit = match permit {
        Ok(permit) => permit,
        Err(_) if fs::symlink_metadata(parent).is_ok_and(|meta| is_redirected_metadata(&meta)) => {
            return Ok(FreshPublication::RequiresLock);
        }
        Err(error) => return Err(error.into()),
    };
    if !no_prior_proof {
        return Ok(FreshPublication::RequiresLock);
    }
    if stopped() {
        return Ok(FreshPublication::Cancelled);
    }
    permit.ensure_namespace_writable(output, flag)?;
    staged.check()?;
    // Both receipt kinds commit the named artifact before publishing its proof.
    match install(staged, output) {
        Ok(true) => Ok(FreshPublication::Installed),
        Ok(false) => Ok(FreshPublication::Contended),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::Unsupported | io::ErrorKind::PermissionDenied
            ) =>
        {
            if fs::symlink_metadata(output).is_ok() {
                Ok(FreshPublication::Contended)
            } else {
                Ok(FreshPublication::RequiresLock)
            }
        }
        Err(error) => Err(write_error(output.to_path_buf(), error)),
    }
}

fn is_absent(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(error) if error.kind() == io::ErrorKind::NotFound)
}
