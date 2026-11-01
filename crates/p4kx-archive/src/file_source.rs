// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

#[cfg(test)]
mod tests;

#[cfg(windows)]
const FILE_SHARE_READ: u32 = 0x0000_0001;
#[cfg(windows)]
const FILE_SHARE_WRITE: u32 = 0x0000_0002;

/// Read-only access to the Archive file by offset, through a pool of file handles.
///
/// Every read is positional, so concurrent readers never share a file cursor. A handle is taken
/// from the pool for one read and returned afterwards; the pool grows to the number of concurrent
/// readers. Renaming and deleting the file is not shared, so every handle reads the same file.
#[derive(Debug)]
pub(crate) struct FileSource {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
    handles: Mutex<Vec<File>>,
}

impl FileSource {
    /// Opens the file read-only; other processes may keep reading and writing it.
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let file = open_shared(path)?;
        let metadata = file.metadata()?;
        Ok(Self {
            path: path.to_path_buf(),
            len: metadata.len(),
            modified: metadata.modified().ok(),
            handles: Mutex::new(vec![file]),
        })
    }

    /// Returns the file length.
    pub(crate) const fn len(&self) -> u64 {
        self.len
    }

    /// Returns the modification time when the file was opened, or `None` when the platform has none.
    pub(crate) const fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    /// Fills `buffer` with the bytes starting at `offset`.
    pub(crate) fn read_into(&self, offset: u64, buffer: &mut [u8]) -> io::Result<()> {
        let file = self.take_handle()?;
        let result = read_exact_at(&file, buffer, offset);
        self.return_handle(file);
        result
    }

    fn take_handle(&self) -> io::Result<File> {
        let pooled = self.pool().pop();
        if let Some(file) = pooled {
            return Ok(file);
        }
        let file = open_shared(&self.path)?;
        if file.metadata()?.len() != self.len {
            return Err(io::Error::new(
                io::ErrorKind::ResourceBusy,
                "the Archive file changed while it was open",
            ));
        }
        Ok(file)
    }

    fn return_handle(&self, file: File) {
        self.pool().push(file);
    }

    fn pool(&self) -> MutexGuard<'_, Vec<File>> {
        self.handles.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn open_shared(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::share_mode(
        &mut options,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
    );
    options.open(path)
}

#[cfg(unix)]
fn read_exact_at(file: &File, buffer: &mut [u8], offset: u64) -> io::Result<()> {
    std::os::unix::fs::FileExt::read_exact_at(file, buffer, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, mut buffer: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !buffer.is_empty() {
        match std::os::windows::fs::FileExt::seek_read(file, buffer, offset) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => {
                buffer = buffer.get_mut(count..).unwrap_or_default();
                offset += count as u64;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
