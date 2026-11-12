// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::fs;
#[cfg(unix)]
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use p4kx_guard::{OwnedDirectoryPermit, OwnedOutputCleanupError};

use crate::manifest::{MESHES_FOLDER, TEXTURES_FOLDER};
use crate::package_destination::check_shape;
use crate::package_paths::{parent_of, sibling_name};
use crate::package_replacement::Replacement;
use crate::rename::{moving_error, rename_to_absent};
use crate::{PackageError, PackageResult, PackageWriteContext, error};

#[cfg(test)]
#[path = "stage_tests.rs"]
mod tests;

/// Checks that the output is absent, empty or a supported package outside installations.
///
/// # Errors
/// Returns protection, package-shape, missing-name or inspection failures.
pub fn check_output(out: &Path) -> PackageResult<()> {
    p4kx_guard::ensure_writable(out)?;
    check_shape(out)
}

/// Checks package output identity and shape with the actual source job's protection.
///
/// # Errors
/// Returns source/target protection, cancellation, package validation or inspection failures.
pub fn check_output_with_context(
    out: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<()> {
    context.writable(out)?;
    check_shape(out)
}

pub(crate) struct Staging<'a> {
    folder: PathBuf,
    out: PathBuf,
    created: Vec<OwnedDirectoryPermit>,
    permit: Option<OwnedDirectoryPermit>,
    committed: bool,
    finished: bool,
    context: PackageWriteContext<'a>,
}

impl<'a> Staging<'a> {
    pub(crate) fn create(out: &Path, context: PackageWriteContext<'a>) -> PackageResult<Self> {
        check_output_with_context(out, &context)?;
        let name = out.file_name().map_or_default(OsStr::to_os_string);
        let folder = parent_of(out).join(sibling_name(&name, "part"));
        let mut staging = Self {
            folder,
            out: out.to_path_buf(),
            created: Vec::new(),
            permit: None,
            committed: false,
            finished: false,
            context,
        };
        if let Err(primary) = staging.prepare() {
            return staging.finish(Err(primary));
        }
        Ok(staging)
    }

    fn prepare(&mut self) -> PackageResult<()> {
        for path in missing_ancestors(&parent_of(&self.out)).iter().rev() {
            self.created.push(self.context.create_directory(path)?);
        }
        self.permit = Some(self.context.create_directory(&self.folder)?);
        for sub in [MESHES_FOLDER, TEXTURES_FOLDER] {
            self.context.create_directory(&self.folder.join(sub))?;
        }
        Ok(())
    }

    pub(crate) fn path(&self) -> &Path {
        &self.folder
    }

    pub(crate) fn complete<T>(self, result: PackageResult<T>) -> PackageResult<T> {
        match result {
            Ok(value) => {
                self.commit()?;
                Ok(value)
            }
            Err(primary) => self.finish(Err(primary)),
        }
    }

    pub(crate) fn commit(mut self) -> PackageResult<()> {
        let result = self.publish();
        self.finish(result)
    }

    fn publish(&mut self) -> PackageResult<()> {
        check_output_with_context(&self.out, &self.context)?;
        for path in [
            self.folder.join(MESHES_FOLDER),
            self.folder.join(TEXTURES_FOLDER),
            self.folder.clone(),
        ] {
            sync_folder(&path).map_err(|source| PackageError::write(&path, source))?;
        }
        let previous = Replacement::move_aside(&self.out, self.context)?;
        if let Err(primary) = self.place() {
            return Err(match previous {
                Some(previous) => previous.restore(primary),
                None => primary,
            });
        }
        self.committed = true;
        if let Some(permit) = &mut self.permit {
            permit
                .rebind_after_rename(&self.out, self.context.cancel_flag())
                .map_err(|cause| PackageError::RetainedOutput {
                    path: self.out.clone(),
                    previous: previous
                        .as_ref()
                        .map(|previous| previous.path().to_path_buf()),
                    cause: Box::new(cause.into()),
                })?;
        }
        if let Some(previous) = previous {
            previous.cleanup()?;
        }
        sync_folder(&parent_of(&self.out)).map_err(|source| PackageError::write(&self.out, source))
    }

    fn place(&self) -> PackageResult<()> {
        self.context.check()?;
        if let Some(permit) = &self.permit {
            permit.ensure_namespace_writable(&self.folder, self.context.cancel_flag())?;
        }
        self.context.writable(&self.out)?;
        rename_to_absent(&self.folder, &self.out).map_err(|source| moving_error(&self.out, source))
    }

    pub(crate) fn finish<T>(mut self, result: PackageResult<T>) -> PackageResult<T> {
        self.finished = true;
        let cleanup = if self.committed {
            Ok(())
        } else {
            self.cleanup()
        };
        error::finish(result, cleanup)
    }

    fn cleanup(&self) -> PackageResult<()> {
        if let Some(permit) = &self.permit {
            permit.cleanup(self.context.cancel_flag())?;
        }
        for permit in self.created.iter().rev() {
            permit
                .ensure_namespace_writable(permit.path(), self.context.cancel_flag())
                .map_err(OwnedOutputCleanupError::from)?;
            fs::remove_dir(permit.path()).map_err(|source| OwnedOutputCleanupError::Io {
                path: permit.path().to_path_buf(),
                source,
            })?;
        }
        Ok(())
    }
}

impl Drop for Staging<'_> {
    fn drop(&mut self) {
        if !self.finished && !self.committed {
            let _retained_on_failure = self.cleanup();
        }
    }
}

fn missing_ancestors(path: &Path) -> Vec<PathBuf> {
    path.ancestors()
        .take_while(|ancestor| {
            !ancestor.as_os_str().is_empty()
                && fs::symlink_metadata(ancestor)
                    .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
        })
        .map(Path::to_path_buf)
        .collect()
}

#[cfg(unix)]
fn sync_folder(folder: &Path) -> io::Result<()> {
    File::open(folder)?.sync_all()
}

#[cfg(not(unix))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "Windows cannot flush directories through File"
)]
const fn sync_folder(_folder: &Path) -> io::Result<()> {
    Ok(())
}
