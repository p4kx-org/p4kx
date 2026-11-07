// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use crate::ExtractionError;
use crate::output_creation::create_folders_protected;

/// The folders below one target that a run created or checked, shared by its worker threads so
/// a folder costs one file system call per file instead of one or more per path component.
#[derive(Debug, Default)]
pub(crate) struct FolderCache {
    known: RwLock<HashSet<PathBuf>>,
}

impl FolderCache {
    /// Returns the folder `relative` below `target`, created if missing.
    ///
    /// A folder seen before is trusted only while it and every folder above it below `target`
    /// still are real folders, each checked by a `symlink_metadata`; otherwise the full creation
    /// runs again, which recreates a deleted folder and refuses a link.
    pub(crate) fn folder_protected(
        &self,
        target: &Path,
        relative: Option<&Path>,
        protection: Option<&crate::ArchiveWriteProtection>,
        cancel: Option<&crate::CancelToken>,
    ) -> Result<PathBuf, ExtractionError> {
        let requested = relative.map_or_else(|| target.to_path_buf(), |path| target.join(path));
        crate::check_protected_output(&requested, protection, cancel)?;
        if let Some(relative) = relative
            && self
                .known
                .read()
                .is_ok_and(|known| known.contains(relative))
            && let Some(folder) = still_real(target, relative)
        {
            return Ok(folder);
        }
        let folder = create_folders_protected(target, relative, protection, cancel)?;
        if let Some(relative) = relative
            && let Ok(mut known) = self.known.write()
        {
            known.insert(relative.to_path_buf());
        }
        Ok(folder)
    }
}

fn still_real(target: &Path, relative: &Path) -> Option<PathBuf> {
    let mut folder = target.to_path_buf();
    for component in relative.components() {
        folder.push(component);
        if !fs::symlink_metadata(&folder).is_ok_and(|metadata| metadata.is_dir()) {
            return None;
        }
    }
    Some(folder)
}
