// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::hash::{Hash as _, Hasher as _};
use std::path::{Path, PathBuf};

use super::paths::is_linked;
use super::{Error, ErrorKind, Limits, check, relative_path};

fn alias(path: &Path, identities: &mut HashMap<u64, Vec<PathBuf>>) -> Result<(), Error> {
    let handle =
        same_file::Handle::from_path(path).map_err(|error| Error::new(ErrorKind::Io, error))?;
    let mut hasher = std::hash::DefaultHasher::new();
    handle.hash(&mut hasher);
    let identity = hasher.finish();
    drop(handle);
    let bucket = identities.entry(identity).or_default();
    for prior in bucket.iter() {
        if same_file::is_same_file(prior, path).map_err(|error| Error::new(ErrorKind::Io, error))? {
            return Err(Error::new(
                ErrorKind::Path,
                "two reference paths alias the same file",
            ));
        }
    }
    bucket.push(path.to_path_buf());
    Ok(())
}

fn insert(
    root: &Path,
    path: PathBuf,
    limits: Limits,
    identities: &mut HashMap<u64, Vec<PathBuf>>,
    files: &mut BTreeMap<String, PathBuf>,
) -> Result<(), Error> {
    let relative = path
        .strip_prefix(root)
        .ok()
        .and_then(Path::to_str)
        .ok_or_else(|| Error::new(ErrorKind::Path, "reference path is not relative UTF-8"))?;
    let key = relative_path(relative, limits.path_bytes)?.to_lowercase();
    alias(&path, identities)?;
    if files.insert(key, path).is_some() {
        return Err(Error::new(
            ErrorKind::Path,
            "case or separator reference collision",
        ));
    }
    Ok(())
}

/// Inventories every direct regular file, with one exact root filename exclusion.
///
/// # Errors
///
/// Returns an error for read failures, links, aliases, cancellation or exceeded bounds.
pub fn inventory(
    root: &Path,
    limits: Limits,
    excluded_root_name: Option<&str>,
    cancelled: impl Fn() -> bool,
) -> Result<BTreeMap<String, PathBuf>, Error> {
    let mut files = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    let mut identities = HashMap::new();
    let mut visited = 0_usize;
    let excluded = excluded_root_name.map(|name| root.join(name));
    while let Some(folder) = folders.pop() {
        check(&cancelled)?;
        for child in fs::read_dir(folder).map_err(|error| Error::new(ErrorKind::Io, error))? {
            check(&cancelled)?;
            let path = child
                .map_err(|error| Error::new(ErrorKind::Io, error))?
                .path();
            visited += 1;
            if visited > limits.entries {
                return Err(Error::new(
                    ErrorKind::Bounds,
                    "reference inventory exceeds its bound",
                ));
            }
            let metadata =
                fs::symlink_metadata(&path).map_err(|error| Error::new(ErrorKind::Io, error))?;
            if is_linked(&metadata) {
                return Err(Error::new(
                    ErrorKind::Path,
                    "reference tree contains a linked path",
                ));
            }
            if metadata.is_dir() {
                folders.push(path);
            } else if metadata.is_file() {
                if excluded.as_ref() != Some(&path) {
                    insert(root, path, limits, &mut identities, &mut files)?;
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Path,
                    "reference tree contains a non-file",
                ));
            }
        }
    }
    Ok(files)
}
