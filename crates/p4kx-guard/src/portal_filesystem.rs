// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path, PathBuf};

use crate::portal_alias::{AliasUncertainty, DocumentGrant, UnknownAlias};
use crate::portal_path::{self, DocumentPath};

#[derive(Debug, PartialEq, Eq)]
struct ObjectIdentity {
    path: PathBuf,
    device: u64,
    inode: u64,
    directory: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GrantSnapshot {
    pub(crate) grant: DocumentGrant,
    objects: Vec<ObjectIdentity>,
}

impl GrantSnapshot {
    pub(crate) fn capture(mount: &[u8], document: &DocumentPath) -> Result<Self, UnknownAlias> {
        let mut root = path_buf(mount);
        root.push(&document.id);
        root.push(std::ffi::OsString::from_vec(document.name.clone()));
        let metadata = fs::symlink_metadata(&root).map_err(filesystem_error)?;
        reject_link(&metadata)?;
        let grant = if metadata.is_dir() {
            DocumentGrant::Directory
        } else if metadata.is_file() {
            DocumentGrant::File
        } else {
            return Err(UnknownAlias::new(AliasUncertainty::Filesystem));
        };
        if grant == DocumentGrant::File && !document.suffix.is_empty() {
            return Err(UnknownAlias::new(AliasUncertainty::FileDescendant));
        }
        let mut objects = vec![identity(root.clone(), &metadata)];
        for component in &document.suffix {
            root.push(std::ffi::OsString::from_vec(component.clone()));
            match fs::symlink_metadata(&root) {
                Ok(metadata) => {
                    reject_link(&metadata)?;
                    objects.push(identity(root.clone(), &metadata));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(error) => return Err(filesystem_error(error)),
            }
        }
        Ok(Self { grant, objects })
    }
}

pub(crate) fn absolute(path: &Path) -> Result<PathBuf, UnknownAlias> {
    let result = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(filesystem_error)?
            .join(path)
    };
    if result.as_os_str().as_bytes().len() > portal_path::MAX_PATH_BYTES {
        return Err(UnknownAlias::new(AliasUncertainty::Limit));
    }
    if result.as_os_str().as_bytes().contains(&0) {
        return Err(UnknownAlias::new(AliasUncertainty::DocumentPath));
    }
    Ok(result)
}

pub(crate) fn existing_ancestor(path: &Path) -> Result<PathBuf, UnknownAlias> {
    let mut ancestor = path;
    let mut suffix = Vec::new();
    loop {
        match fs::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for component in suffix.into_iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let Some(Component::Normal(name)) = ancestor.components().next_back() else {
                    return Err(UnknownAlias::new(AliasUncertainty::DocumentPath));
                };
                suffix.push(name.to_os_string());
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| UnknownAlias::new(AliasUncertainty::DocumentPath))?;
            }
            Err(error) => return Err(filesystem_error(error)),
        }
    }
}

pub(crate) fn path_buf(bytes: &[u8]) -> PathBuf {
    PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()))
}

fn reject_link(metadata: &fs::Metadata) -> Result<(), UnknownAlias> {
    if metadata.file_type().is_symlink() {
        return Err(UnknownAlias::new(AliasUncertainty::SymbolicLink));
    }
    Ok(())
}

fn identity(path: PathBuf, metadata: &fs::Metadata) -> ObjectIdentity {
    ObjectIdentity {
        path,
        device: metadata.dev(),
        inode: metadata.ino(),
        directory: metadata.is_dir(),
    }
}

fn filesystem_error(error: io::Error) -> UnknownAlias {
    UnknownAlias::with_source(AliasUncertainty::Filesystem, error)
}

#[cfg(test)]
mod tests;
