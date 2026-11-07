// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::offline_seam::{self, ErrorKind, Limits};

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{MANIFEST_NAME, MAX_ARTIFACTS, MAX_PATH_BYTES};

fn error(error: &offline_seam::Error) -> ReferenceError {
    let code = if error.kind == ErrorKind::Bounds {
        Code::Bounds
    } else {
        Code::Path
    };
    ReferenceError::new(code, error.to_string())
}

pub(crate) fn relative(text: &str) -> Result<String> {
    offline_seam::relative_path(text, MAX_PATH_BYTES)
        .map_err(|error| ReferenceError::new(Code::Path, error.to_string()))
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

pub(crate) fn root(path: &Path) -> Result<PathBuf> {
    offline_seam::direct_root(path).map_err(|failure| error(&failure))
}

pub(crate) fn resolve(root: &Path, relative_path: &str) -> Result<PathBuf> {
    offline_seam::contained_file(root, relative_path, MAX_PATH_BYTES)
        .map_err(|failure| error(&failure))
}

pub(crate) fn inventory(root: &Path) -> Result<BTreeMap<String, PathBuf>> {
    offline_seam::inventory(
        root,
        Limits {
            path_bytes: MAX_PATH_BYTES,
            entries: MAX_ARTIFACTS,
        },
        Some(MANIFEST_NAME),
        || false,
    )
    .map_err(|failure| error(&failure))
}

pub(crate) fn for_write(root: &Path, name: &str) -> Result<PathBuf> {
    let relative = relative(name)?;
    let mut path = root.to_path_buf();
    let components: Vec<_> = relative.split('/').collect();
    for (index, component) in components.iter().enumerate() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if offline_seam::is_linked(&metadata) => {
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
