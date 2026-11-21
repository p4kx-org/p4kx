// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::Path;

use crate::{AliasUncertainty, UnknownAlias};

const MAX_MEMBERSHIP_CHILDREN: usize = 4096;

pub(crate) fn contains_installation(parent: &Path) -> Result<bool, UnknownAlias> {
    if has_game_folder(parent)? {
        return Ok(true);
    }
    let children = fs::read_dir(parent).map_err(filesystem_error)?;
    for (index, child) in children.enumerate() {
        if index >= MAX_MEMBERSHIP_CHILDREN {
            return Err(UnknownAlias::new(AliasUncertainty::Limit));
        }
        let child = child.map_err(filesystem_error)?;
        let metadata = fs::metadata(child.path()).map_err(filesystem_error)?;
        if metadata.is_dir() && has_game_folder(&child.path())? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn has_game_folder(parent: &Path) -> Result<bool, UnknownAlias> {
    match fs::metadata(parent.join("Bin64")) {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(filesystem_error(error)),
    }
}

fn filesystem_error(error: io::Error) -> UnknownAlias {
    UnknownAlias::with_source(AliasUncertainty::Filesystem, error)
}
