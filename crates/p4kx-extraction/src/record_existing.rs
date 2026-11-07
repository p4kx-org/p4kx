// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::Path;

use crate::counting::holds_sized;
use crate::write::{read_existing_error, write_error};
use crate::{ConflictPolicy, ExtractionError};

/// Applies the Record conflict policy to one existing output and its prospective bytes.
pub(crate) fn keep(
    destination: &Path,
    policy: ConflictPolicy,
    bytes: &[u8],
    read: &mut Vec<u8>,
) -> Result<bool, ExtractionError> {
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => return Err(read_existing_error(destination, source)),
    };
    if !metadata.is_file() {
        return Err(write_error(
            destination.to_path_buf(),
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "a folder or link is in the way",
            ),
        ));
    }
    match policy {
        ConflictPolicy::Skip => Ok(true),
        ConflictPolicy::Overwrite => Ok(false),
        ConflictPolicy::IfChanged => holds_sized(
            destination,
            metadata.len(),
            bytes.len() as u64,
            || crc32c::crc32c(bytes),
            read,
        )
        .map_err(|source| read_existing_error(destination, source)),
    }
}
