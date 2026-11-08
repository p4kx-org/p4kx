// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use crate::output_file;
use crate::{ConversionError, DiskTextures};

const ALPHA_SUFFIX: &str = "a";

/// A disk header and sibling names resolved through the same conversion run.
#[derive(Clone, Debug)]
pub(crate) struct DiskParts {
    header: PathBuf,
    name: PathBuf,
    base: PathBuf,
    suffix: &'static str,
    files: DiskTextures,
}

impl DiskParts {
    pub(crate) fn new(files: &DiskTextures, path: &Path) -> io::Result<Self> {
        let header = std::path::absolute(path)?;
        files.remember_exact(&header)?;
        let is_alpha = path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case(ALPHA_SUFFIX));
        Ok(Self {
            name: path.to_path_buf(),
            base: if is_alpha {
                header.with_extension("")
            } else {
                header.clone()
            },
            header,
            suffix: if is_alpha { ALPHA_SUFFIX } else { "" },
            files: files.clone(),
        })
    }

    pub(crate) fn alpha_candidate(&self) -> Option<Self> {
        (self.suffix != ALPHA_SUFFIX).then(|| Self {
            header: with_suffix(&self.base, ".a"),
            name: with_suffix(&self.name, ".a"),
            base: self.base.clone(),
            suffix: ALPHA_SUFFIX,
            files: self.files.clone(),
        })
    }

    pub(crate) fn header(&self) -> &Path {
        &self.header
    }

    pub(crate) fn name(&self) -> &Path {
        &self.name
    }

    pub(crate) fn header_exists(&self) -> io::Result<bool> {
        self.files.exists(&self.header)
    }

    pub(crate) fn open_header(&self) -> io::Result<File> {
        self.files.open_file(&self.header)
    }

    pub(crate) fn part_len(&self, index: u32) -> io::Result<Option<u64>> {
        self.files.part_len(&self.part_path(index))
    }

    pub(crate) fn open_part(&self, index: u32) -> io::Result<File> {
        self.files.open_file(&self.part_path(index))
    }

    pub(crate) fn input_files(&self, count: u32) -> Result<Vec<PathBuf>, ConversionError> {
        let mut paths = vec![
            self.files
                .resolve_path(&self.header)
                .map_err(|source| output_file::read_error(&self.header, source))?
                .unwrap_or_else(|| self.header.clone()),
        ];
        for index in 1..=count {
            let requested = self.part_path(index);
            paths.push(
                self.files
                    .resolve_path(&requested)
                    .map_err(|source| output_file::read_error(&requested, source))?
                    .unwrap_or(requested),
            );
        }
        Ok(paths)
    }

    fn part_path(&self, index: u32) -> PathBuf {
        with_suffix(&self.base, &format!(".{index}{}", self.suffix))
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(suffix);
    PathBuf::from(name)
}
