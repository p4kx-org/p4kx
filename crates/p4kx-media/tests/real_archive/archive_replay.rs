// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, Entry};
use p4kx_media::{ExportOutcome, Index};
use serde::de::DeserializeOwned;

use super::TestResult;
use super::shader_accounting::require;

pub(super) fn generation(
    output: &Path,
    outcome: ExportOutcome,
) -> TestResult<(Box<Index>, PathBuf)> {
    let (index, parent) = match outcome {
        ExportOutcome::Published { index, .. } => (index, output.to_path_buf()),
        ExportOutcome::Incomplete { index, diagnostic } => (
            index,
            diagnostic
                .parent()
                .ok_or("diagnostic generation directory")?
                .to_path_buf(),
        ),
        _ => return Err("verification did not run an export".into()),
    };
    let base = match index.artifact_base.as_str() {
        "." => parent,
        relative => parent.join(relative),
    };
    Ok((index, base))
}

pub(super) fn compare_original(archive: &Archive, entry: &Entry, retained: &[u8]) -> TestResult {
    let mut original = archive.open_entry_verified(entry)?;
    let mut buffer = [0_u8; 16 << 10];
    for bytes in retained.chunks(buffer.len()) {
        let observed = buffer.get_mut(..bytes.len()).ok_or("comparison buffer")?;
        original.read_exact(observed)?;
        require(
            observed == bytes,
            "retained payload matches original verified bytes",
        )?;
    }
    require(
        original.read(&mut buffer)? == 0,
        "retained payload includes the complete Entry",
    )
}

pub(super) struct Lines {
    input: BufReader<File>,
    line: String,
}

impl Lines {
    pub(super) fn open(path: &Path) -> TestResult<Self> {
        Ok(Self {
            input: BufReader::new(File::open(path)?),
            line: String::new(),
        })
    }

    pub(super) fn next<T: DeserializeOwned>(&mut self) -> TestResult<Option<T>> {
        self.line.clear();
        if self.input.read_line(&mut self.line)? == 0 {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&self.line)?))
    }
}
