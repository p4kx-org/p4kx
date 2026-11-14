// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Conversion of a folder of extracted files.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::dataforge_pick::by_name_then_content;
use crate::extract::{Rules, extract_files};
use crate::{ExtractOptions, ExtractionReport, FolderError, FolderSelection, FolderSource};

/// Converts the files of `selection`, made by [`FolderSource::select`] from `source`, into the
/// folder `target` with the same relative paths, as `extract` with Conversion on writes the same
/// Entries.
///
/// A texture whose header the selection's exclusion matched is left out with its parts; a part it
/// matched alone is not written, but a texture whose header remains is still built from it.
///
/// A file Conversion has nothing to do with is counted in
/// [`ExtractionReport::not_converted`], or with `copy_other` written unchanged. The files and
/// folders the scan did not list, such as links, are among the report's failures. Verification
/// and the Archive watch of `options` do not apply to a folder, and Conversion is always on.
///
/// # Errors
///
/// Returns [`FolderError::Overlap`] when `target` and the folder of `source` lie inside one
/// another, [`FolderError::Installation`] when `target` lies inside a Star Citizen installation,
/// and [`FolderError::Read`] when `target` cannot be created; nothing is written then.
pub fn convert_folder(
    source: &FolderSource,
    selection: &FolderSelection<'_>,
    copy_other: bool,
    target: &Path,
    options: &ExtractOptions<'_>,
) -> Result<ExtractionReport, FolderError> {
    ensure_disjoint(source.root(), target)?;
    p4kx_guard::ensure_writable(target)?;
    fs::create_dir_all(target).map_err(|error| FolderError::Read {
        path: target.to_path_buf(),
        source: error,
    })?;
    let options = ExtractOptions {
        convert: true,
        verify: false,
        watch: None,
        ..*options
    };
    let entries = &selection.entries;
    let rules = Rules {
        entry_target: crate::write::EntryTarget::Source,
        copy_other,
        pick: &by_name_then_content,
    };
    let mut report = extract_files(source, entries, target, &options, &rules);
    report.failures.splice(0..0, source.rejected_failures());
    Ok(report)
}

/// Fails when `input` and `output` are the same folder or one lies inside the other.
fn ensure_disjoint(input: &Path, output: &Path) -> Result<(), FolderError> {
    let overlap = || FolderError::Overlap {
        input: input.to_path_buf(),
        output: output.to_path_buf(),
    };
    let resolved = resolve(output).map_err(|error| FolderError::Read {
        path: output.to_path_buf(),
        source: error,
    })?;
    if resolved.starts_with(input) || input.starts_with(&resolved) {
        return Err(overlap());
    }
    Ok(())
}

/// Returns the absolute path with every link resolved in the part that exists, followed by the
/// part that does not exist yet.
fn resolve(path: &Path) -> io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut missing = Vec::new();
    let mut head = absolute.as_path();
    loop {
        match fs::canonicalize(head) {
            Ok(mut resolved) => {
                resolved.extend(missing.iter().rev());
                return Ok(resolved);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let (Some(name), Some(parent)) = (head.file_name(), head.parent()) else {
                    return Err(error);
                };
                missing.push(name);
                head = parent;
            }
            Err(error) => return Err(error),
        }
    }
}
