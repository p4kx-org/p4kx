// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;

use p4kx_extraction::{Archive, Entry, PathIndex};

use super::MAX_INTERIOR_FILE_LEN;

/// Why a file of a container cannot be read.
pub(super) enum FileFailure {
    TooLarge(u64),
    Unreadable(String),
    Fatal(Box<dyn Error + Send + Sync>),
}

impl FileFailure {
    pub(super) fn reason(&self) -> String {
        match self {
            Self::TooLarge(size) => {
                format!("it holds {size} bytes, more than the limit of {MAX_INTERIOR_FILE_LEN}")
            }
            Self::Unreadable(reason) => reason.clone(),
            Self::Fatal(error) => error.to_string(),
        }
    }
}

/// Reads the whole data of `entry` unless it is larger than [`MAX_INTERIOR_FILE_LEN`].
pub(super) fn read_file(archive: &Archive, entry: &Entry) -> Result<Vec<u8>, FileFailure> {
    let size = entry.uncompressed_size();
    if size > MAX_INTERIOR_FILE_LEN {
        return Err(FileFailure::TooLarge(size));
    }
    let reader = archive.open_entry(entry).map_err(|error| {
        if error.is_key_required() || error.is_wrong_key() {
            FileFailure::Fatal(Box::new(error))
        } else {
            FileFailure::Unreadable(error.to_string())
        }
    })?;
    let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or_default());
    reader
        .take(MAX_INTERIOR_FILE_LEN)
        .read_to_end(&mut bytes)
        .map_err(|error| FileFailure::Unreadable(error.to_string()))?;
    Ok(bytes)
}

/// Returns `path` with `\` as separator, in lower case and without a leading separator.
pub(super) fn path_key(path: &str) -> String {
    path.trim_start_matches(['/', '\\'])
        .chars()
        .map(|c| {
            if c == '/' {
                '\\'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// Returns the name of the file at `path` without folders.
pub(super) fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Returns the name of the file at `path` without folders and extension.
pub(crate) fn stem(path: &str) -> &str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}

/// Returns whether `path` is a plain relative path: no empty, `.` or `..` component and no drive
/// or stream marker.
pub(crate) fn is_plain(path: &str) -> bool {
    path.split(['/', '\\'])
        .skip_while(|part| part.is_empty())
        .all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.chars().any(|c| c.is_control() || c == ':')
        })
}

/// Finds the first existing candidate within the current Archive.
pub(super) fn find_first<'archive>(
    index: &PathIndex<'archive>,
    candidates: impl IntoIterator<Item: AsRef<str>>,
) -> Option<&'archive Entry> {
    candidates
        .into_iter()
        .find_map(|candidate| index.find(candidate.as_ref()))
}
