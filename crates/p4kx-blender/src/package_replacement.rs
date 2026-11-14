// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_guard::OwnedDirectoryPermit;

use crate::package_destination::check_shape;
use crate::package_paths::{parent_of, sibling_name};
use crate::rename::{moving_error, rename_to_absent};
use crate::{PackageError, PackageResult, PackageWriteContext};

pub(crate) struct Replacement<'a> {
    permit: OwnedDirectoryPermit,
    out: PathBuf,
    context: PackageWriteContext<'a>,
}

impl<'a> Replacement<'a> {
    pub(crate) fn move_aside(
        out: &Path,
        context: PackageWriteContext<'a>,
    ) -> PackageResult<Option<Self>> {
        match fs::symlink_metadata(out) {
            Ok(_) => {}
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(PackageError::write(out, source)),
        }
        let mut permit = p4kx_guard::validated_output_directory(
            out,
            context.protection(),
            context.cancel_flag(),
            |root| check_shape(root).map_err(io::Error::other),
        )?;
        let name = out.file_name().map_or_default(OsStr::to_os_string);
        let old = parent_of(out).join(sibling_name(&name, "old"));
        context.writable(&old)?;
        permit.ensure_namespace_writable(out, context.cancel_flag())?;
        rename_to_absent(out, &old).map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                PackageError::BackupExists { path: old.clone() }
            } else {
                moving_error(&old, source)
            }
        })?;
        permit
            .rebind_after_rename(&old, context.cancel_flag())
            .map_err(|cause| PackageError::RetainedOutput {
                path: old,
                previous: None,
                cause: Box::new(cause.into()),
            })?;
        Ok(Some(Self {
            permit,
            out: out.to_path_buf(),
            context,
        }))
    }

    pub(crate) fn path(&self) -> &Path {
        self.permit.path()
    }

    pub(crate) fn cleanup(self) -> PackageResult<()> {
        self.permit.cleanup(self.context.cancel_flag())?;
        Ok(())
    }

    pub(crate) fn restore(mut self, replacement: PackageError) -> PackageError {
        let checked = self.context.cleanup_writable(&self.out).and_then(|()| {
            self.permit
                .ensure_namespace_writable(self.path(), self.context.cancel_flag())?;
            Ok(())
        });
        if let Err(cause) = checked {
            return PackageError::RestoreRefused {
                path: self.out,
                old: self.permit.path().to_path_buf(),
                cause: Box::new(cause),
                replacement: Box::new(replacement),
            };
        }
        if let Err(source) = rename_to_absent(self.path(), &self.out) {
            return PackageError::CleanupFailed {
                primary: Box::new(replacement),
                cleanup: Box::new(PackageError::NotRestored {
                    path: self.out,
                    old: self.permit.path().to_path_buf(),
                    source,
                }),
            };
        }
        if let Err(cause) = self
            .permit
            .rebind_after_rename(&self.out, self.context.cancel_flag())
        {
            return PackageError::CleanupFailed {
                primary: Box::new(replacement),
                cleanup: Box::new(PackageError::RetainedOutput {
                    path: self.out,
                    previous: None,
                    cause: Box::new(cause.into()),
                }),
            };
        }
        replacement
    }
}
