// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The animation files of the Archive below the folders of a skeleton.

use std::collections::HashSet;

use rayon::prelude::*;

use crate::ArchiveModels;
use crate::path_match::starts_with_path;

/// The clip files below some folders.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ClipFiles {
    /// The file names of the `.caf` files without extension, lower case.
    pub(super) stems: HashSet<String>,
    /// Whether a `.dba` database lies there, whose clip names show only inside it.
    pub(super) has_database: bool,
}

/// Lists the clip files of the Archive below the folders `folders`, paths below `Data`, in one scan
/// of its entries.
pub(super) fn below(models: &ArchiveModels<'_>, folders: &[&str]) -> ClipFiles {
    let prefixes: Vec<String> = folders
        .iter()
        .map(|folder| format!("data/{}/", folder.trim_matches(['/', '\\'])))
        .collect();
    let files: Vec<&str> = models
        .archive
        .entries()
        .par_iter()
        .map(p4kx_extraction::Entry::path)
        .filter(|path| prefixes.iter().any(|prefix| starts_with_path(path, prefix)))
        .collect();
    let mut found = ClipFiles::default();
    for path in files {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        let Some((stem, extension)) = name.rsplit_once('.') else {
            continue;
        };
        if extension.eq_ignore_ascii_case("caf") {
            found.stems.insert(stem.to_ascii_lowercase());
        } else if extension.eq_ignore_ascii_case("dba") {
            found.has_database = true;
        }
    }
    found
}
