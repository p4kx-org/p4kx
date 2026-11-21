// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn launcher(root: &Path, user: &str) -> io::Result<PathBuf> {
    let folder = root
        .join("drive_c/users")
        .join(user)
        .join("AppData/Roaming/rsilauncher/logs");
    fs::create_dir_all(&folder)?;
    Ok(folder)
}

pub(super) fn patch(version: &str) -> String {
    patch_change(version, "42")
}

pub(super) fn patch_change(version: &str, change: &str) -> String {
    format!("(SC LIVE {version}-live.{change})\n")
}

pub(super) struct FailingInput {
    first: Cursor<Vec<u8>>,
    failure: io::ErrorKind,
}

impl FailingInput {
    pub(super) fn new(bytes: Vec<u8>, failure: io::ErrorKind) -> Self {
        Self {
            first: Cursor::new(bytes),
            failure,
        }
    }
}

impl Read for FailingInput {
    fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
        let count = self.first.read(target)?;
        if count == 0 {
            return Err(io::Error::new(self.failure, "synthetic opened-log failure"));
        }
        Ok(count)
    }
}

impl Seek for FailingInput {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.first.seek(position)
    }
}
