// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

pub use p4kx_foundation::{FOLDERS_ROOT_VARIABLE, FolderKind, XdgEnvironment, base_folder};

pub(crate) const APP_FOLDER: &str = "P4KX";

/// Returns the `P4KX` folder for caches, `None` when the platform has no cache folder.
#[must_use]
pub fn cache_folder() -> Option<PathBuf> {
    base_folder(FolderKind::Cache).map(|base| base.join(APP_FOLDER))
}

/// Returns the `P4KX` folder for data the user keeps, `None` when the platform has none.
#[must_use]
pub fn data_folder() -> Option<PathBuf> {
    base_folder(FolderKind::Data).map(|base| base.join(APP_FOLDER))
}

/// Creates `folder` and its missing parents, readable by the user alone (mode 0700) on Unix.
///
/// An existing folder keeps its permissions.
///
/// # Errors
///
/// Returns the I/O error when a folder cannot be created.
pub(crate) fn create_private_folder(folder: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(folder)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(folder)
    }
}
