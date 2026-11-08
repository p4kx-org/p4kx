// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_archive::Entry;
use p4kx_model::{CompanionError, MaterialRequest};
use thiserror::Error;

use crate::PathIndex;

#[derive(Debug, Error)]
enum MaterialReferenceError {
    #[error("the material reference is not an admitted virtual Archive path")]
    Unsupported,
    #[error("the authoring reference has more than one Data root")]
    AmbiguousRoot,
    #[error("the mapped Archive target {target} matches more than one Entry")]
    AmbiguousTarget { target: String },
}

pub(crate) fn request(name: &str) -> Result<MaterialRequest, CompanionError> {
    let trimmed = name.trim();
    if name.chars().any(char::is_control) || trimmed.starts_with('\\') {
        return Err(MaterialReferenceError::Unsupported.into());
    }
    let normalized = trimmed.replace('\\', "/");
    let relative = if let Some(authoring) = normalized.strip_prefix("//") {
        authoring_relative(authoring)?
    } else if let Some(virtual_root) = normalized.strip_prefix('/') {
        virtual_root
    } else {
        normalized.as_str()
    };
    let request = MaterialRequest::new(relative)?;
    Ok(if normalized.starts_with('/') {
        request.at_data_root()
    } else {
        request
    })
}

fn authoring_relative(name: &str) -> Result<&str, CompanionError> {
    MaterialRequest::new(name)?;
    let mut root = None;
    for (position, part) in name.split('/').enumerate() {
        if part.eq_ignore_ascii_case("Data") && root.replace(position).is_some() {
            return Err(MaterialReferenceError::AmbiguousRoot.into());
        }
    }
    let Some(position) = root.filter(|position| *position >= 2) else {
        return Err(MaterialReferenceError::Unsupported.into());
    };
    name.splitn(position + 2, '/')
        .nth(position + 1)
        .filter(|relative| !relative.is_empty())
        .ok_or_else(|| MaterialReferenceError::Unsupported.into())
}

pub(crate) fn target(
    index: &PathIndex<'_>,
    folder: &str,
    request: &MaterialRequest,
) -> Result<String, CompanionError> {
    if request.searches_sibling() && !request.path().contains('/') {
        let sibling = if folder.is_empty() {
            format!("{}.mtl", request.path())
        } else {
            format!("{folder}/{}.mtl", request.path())
        };
        if let Some(entry) = unique_entry(index, &sibling)? {
            return Ok(entry.path().to_owned());
        }
    }
    let target = format!("Data/{}.mtl", request.path());
    Ok(unique_entry(index, &target)?.map_or(target, |entry| entry.path().to_owned()))
}

pub(crate) fn unique_entry<'a>(
    index: &PathIndex<'a>,
    path: &str,
) -> Result<Option<&'a Entry>, CompanionError> {
    let mut matches = index.matching_entries_with(path, |_| Ok::<(), Infallible>(()))?;
    let first = matches.next().transpose()?;
    if matches.next().transpose()?.is_some() {
        return Err(MaterialReferenceError::AmbiguousTarget {
            target: path.to_owned(),
        }
        .into());
    }
    Ok(first)
}
