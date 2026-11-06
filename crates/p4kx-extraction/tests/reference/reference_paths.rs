// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{MANIFEST_NAME, MAX_ARTIFACTS, MAX_PATH_BYTES};

pub(crate) fn relative(text: &str) -> Result<String> {
    let path = text.replace('\\', "/");
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.starts_with('/')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ReferenceError::new(
            Code::Path,
            "not a bounded relative artifact/source path",
        ));
    }
    Ok(path)
}

pub(crate) fn key(text: &str) -> Result<String> {
    relative(text).map(|path| path.to_lowercase())
}

pub(crate) fn pointer(text: &str) -> Result<()> {
    if !text.starts_with('/') || text.len() > MAX_PATH_BYTES {
        return Err(ReferenceError::new(
            Code::Path,
            "invalid bounded JSON subresource pointer",
        ));
    }
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character == '~' && !matches!(characters.next(), Some('0' | '1')) {
            return Err(ReferenceError::new(
                Code::Path,
                "invalid JSON pointer escape",
            ));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn linked(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const REPARSE_POINT_ATTRIBUTE: u32 = 0x400;
    metadata.file_attributes() & REPARSE_POINT_ATTRIBUTE != 0
}

#[cfg(not(windows))]
fn linked(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

pub(crate) fn root(path: &Path) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
    if !metadata.is_dir() || linked(&metadata) {
        return Err(ReferenceError::new(
            Code::Path,
            "reference root must be a direct directory",
        ));
    }
    fs::canonicalize(path).map_err(|error| ReferenceError::new(Code::Path, error.to_string()))
}

pub(crate) fn resolve(root: &Path, relative_path: &str) -> Result<PathBuf> {
    let relative_path = relative(relative_path)?;
    let mut path = root.to_path_buf();
    for component in relative_path.split('/') {
        path.push(component);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
        if linked(&metadata) {
            return Err(ReferenceError::new(
                Code::Path,
                "linked artifact path is not accepted",
            ));
        }
    }
    let resolved = fs::canonicalize(&path)
        .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(ReferenceError::new(
            Code::Path,
            "artifact resolves outside root or is not a file",
        ));
    }
    Ok(resolved)
}

fn check_alias(path: &Path, identities: &mut HashMap<u64, Vec<PathBuf>>) -> Result<()> {
    use std::hash::{Hash as _, Hasher as _};
    let handle = same_file::Handle::from_path(path)
        .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
    let mut hasher = std::hash::DefaultHasher::new();
    handle.hash(&mut hasher);
    let identity = hasher.finish();
    drop(handle);
    let bucket = identities.entry(identity).or_default();
    for prior in bucket.iter() {
        if same_file::is_same_file(prior, path)
            .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?
        {
            return Err(ReferenceError::new(
                Code::Path,
                "two artifact paths alias the same file",
            ));
        }
    }
    bucket.push(path.to_path_buf());
    Ok(())
}

pub(crate) fn inventory(root: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut files = BTreeMap::new();
    let mut folders = vec![root.to_path_buf()];
    let mut identities = HashMap::new();
    let mut visited = 0_usize;
    while let Some(folder) = folders.pop() {
        let children = fs::read_dir(&folder)
            .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
        for child in children {
            let child =
                child.map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
            visited += 1;
            if visited > MAX_ARTIFACTS {
                return Err(ReferenceError::new(
                    Code::Bounds,
                    "reference tree has too many entries",
                ));
            }
            let path = child.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
            if linked(&metadata) {
                return Err(ReferenceError::new(
                    Code::Path,
                    "reference tree contains a linked path",
                ));
            }
            if metadata.is_dir() {
                folders.push(path);
            } else if metadata.is_file() {
                if path == root.join(MANIFEST_NAME) {
                    continue;
                }
                let name = path
                    .strip_prefix(root)
                    .ok()
                    .and_then(Path::to_str)
                    .ok_or_else(|| {
                        ReferenceError::new(Code::Path, "artifact name is not relative UTF-8")
                    })?;
                let key = key(name)?;
                check_alias(&path, &mut identities)?;
                if files.insert(key, path).is_some() {
                    return Err(ReferenceError::new(
                        Code::Path,
                        "case or separator artifact collision",
                    ));
                }
            } else {
                return Err(ReferenceError::new(
                    Code::Path,
                    "reference tree contains a non-file artifact",
                ));
            }
        }
    }
    Ok(files)
}

pub(crate) fn for_write(root: &Path, name: &str) -> Result<PathBuf> {
    let relative = relative(name)?;
    let mut path = root.to_path_buf();
    let components: Vec<_> = relative.split('/').collect();
    for (index, component) in components.iter().enumerate() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if linked(&metadata) => {
                return Err(ReferenceError::new(
                    Code::Path,
                    "linked output ancestor is not accepted",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if index + 1 != components.len() {
                    fs::create_dir(&path)
                        .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
                }
            }
            Err(error) => return Err(ReferenceError::new(Code::Path, error.to_string())),
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| ReferenceError::new(Code::Path, "output has no parent"))?;
    let parent = fs::canonicalize(parent)
        .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))?;
    if !parent.starts_with(root) {
        return Err(ReferenceError::new(
            Code::Path,
            "output resolves outside reference root",
        ));
    }
    Ok(path)
}
