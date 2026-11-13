// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, Metadata};
use std::path::{Component, Path, PathBuf};

use p4kx_extraction::is_redirected_metadata;

use crate::MediaError;
use crate::checkpoint::Checkpoint;
use crate::error;

pub(crate) fn ensure_directory(path: &Path, checkpoint: &Checkpoint<'_>) -> Result<(), MediaError> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(MediaError::Destination(
            "parent traversal in output path".into(),
        ));
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        checkpoint.check()?;
        match inspect(&current)? {
            Some(metadata) if metadata.is_dir() => {}
            Some(_) => return Err(not_plain(&current)),
            None => {
                p4kx_extraction::create_output_directory(
                    &current,
                    Some(checkpoint.protection),
                    checkpoint.cancel.flag(),
                )?;
            }
        }
    }
    p4kx_extraction::validated_output_directory(
        path,
        Some(checkpoint.protection),
        checkpoint.cancel.flag(),
        |_| Ok(()),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "output_path_tests.rs"]
mod tests;

pub(crate) fn file(path: &Path) -> Result<bool, MediaError> {
    match existing(path)? {
        Some(metadata) if metadata.is_file() => Ok(true),
        Some(_) => Err(not_plain(path)),
        None => Ok(false),
    }
}

pub(crate) fn directory(path: &Path) -> Result<bool, MediaError> {
    match existing(path)? {
        Some(metadata) if metadata.is_dir() => Ok(true),
        Some(_) => Err(not_plain(path)),
        None => Ok(false),
    }
}

fn existing(path: &Path) -> Result<Option<Metadata>, MediaError> {
    let metadata = inspect(path)?;
    for ancestor in path.ancestors().skip(1) {
        if let Some(metadata) = inspect(ancestor)?
            && !metadata.is_dir()
        {
            return Err(not_plain(ancestor));
        }
    }
    Ok(metadata)
}

fn inspect(path: &Path) -> Result<Option<Metadata>, MediaError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if is_redirected_metadata(&metadata) => inspect_redirected(path),
        Ok(metadata) => Ok(Some(metadata)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(error::io(path, source)),
    }
}

fn inspect_redirected(path: &Path) -> Result<Option<Metadata>, MediaError> {
    #[cfg(target_os = "macos")]
    {
        let target = match path.to_str() {
            Some("/var") => Path::new("/private/var"),
            Some("/tmp") => Path::new("/private/tmp"),
            _ => return Err(not_plain(path)),
        };
        let link = fs::read_link(path).map_err(|source| error::io(path, source))?;
        if Path::new("/").join(link) == target
            && let Some(metadata) = existing(target)?
            && metadata.is_dir()
        {
            return Ok(Some(metadata));
        }
    }
    Err(not_plain(path))
}

fn not_plain(path: &Path) -> MediaError {
    MediaError::Destination(format!("{} is not a plain output path", path.display()))
}
