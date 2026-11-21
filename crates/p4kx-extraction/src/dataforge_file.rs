// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use p4kx_dataforge::{SNIFF_LEN, looks_like_dataforge};

use crate::dataforge::check_size_within;
use crate::{
    ArchiveChanged, DataForgeEntry, DataForgeFileOrigin, DataForgeLoadError, DataForgeOrigin,
    FileStamp, MAX_DATAFORGE_SIZE,
};

/// Returns whether the file at `path` starts like a DataForge database, whatever its name.
///
/// A file of any version passes, a supported one or not; a file shorter than the header is none.
///
/// # Errors
///
/// Returns the error of opening or reading the file.
pub fn is_dataforge_file(path: &Path) -> io::Result<bool> {
    let file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut prefix = Vec::with_capacity(SNIFF_LEN);
    file.take(SNIFF_LEN as u64).read_to_end(&mut prefix)?;
    Ok(looks_like_dataforge(&prefix, len))
}

impl DataForgeEntry {
    /// Reads the DataForge database stored in the file at `file`, e.g. a `Game2.dcb` kept from an
    /// earlier patch; its [`path`](Self::path) is the file name. The file is opened read-only.
    ///
    /// # Errors
    ///
    /// Returns [`DataForgeLoadError::TooLarge`] when the file is larger than
    /// [`MAX_DATAFORGE_SIZE`](crate::MAX_DATAFORGE_SIZE), and [`DataForgeLoadError::Read`] when
    /// it cannot be opened or read.
    pub fn read_file(file: &Path) -> Result<Self, DataForgeLoadError> {
        Self::read_file_until(file, &|| false)
    }

    /// Reads the DataForge file like [`DataForgeEntry::read_file`], polling `stop` before each
    /// mebibyte.
    ///
    /// # Errors
    ///
    /// The same as [`DataForgeEntry::read_file`], and [`DataForgeLoadError::Cancelled`] once
    /// `stop` returned `true`.
    pub fn read_file_until(
        file: &Path,
        stop: &dyn Fn() -> bool,
    ) -> Result<Self, DataForgeLoadError> {
        Self::read_file_within(file, MAX_DATAFORGE_SIZE, stop)
    }

    /// Reads the DataForge file like [`DataForgeEntry::read_file_until`], refusing a file larger
    /// than `limit` bytes.
    pub(crate) fn read_file_within(
        file: &Path,
        limit: u64,
        stop: &dyn Fn() -> bool,
    ) -> Result<Self, DataForgeLoadError> {
        let name = file
            .file_name()
            .unwrap_or(file.as_os_str())
            .to_string_lossy()
            .into_owned();
        let read = |source| DataForgeLoadError::Read {
            path: name.clone(),
            source,
        };
        let source_path = std::path::absolute(file).map_err(read)?;
        let handle = File::open(&source_path).map_err(read)?;
        let opened = handle.metadata().map_err(read)?;
        let size = opened.len();
        check_size_within(&name, size, limit)?;
        let mut entry = Self::read_stream(name.clone(), size, handle, stop)?;
        let now = std::fs::metadata(&source_path).map_err(read)?;
        let stamp = FileStamp::from_metadata(&opened);
        if FileStamp::from_metadata(&now) != stamp {
            return Err(DataForgeLoadError::ArchiveChanged(ArchiveChanged::of(file)));
        }
        entry.set_origin(DataForgeOrigin::File(DataForgeFileOrigin::new(
            source_path,
            stamp,
        )));
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;

    #[test]
    fn a_file_over_the_limit_is_refused_before_it_is_read() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("big.dcb");
        std::fs::write(&file, [0_u8; 17])?;

        let result = DataForgeEntry::read_file_within(&file, 16, &|| false);

        assert!(matches!(
            result,
            Err(DataForgeLoadError::TooLarge { size: 17, limit: 16, ref path }) if path == "big.dcb"
        ));
        Ok(())
    }

    #[test]
    fn a_file_at_the_limit_is_read() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let file = dir.path().join("exact.dcb");
        std::fs::write(&file, [0_u8; 16])?;

        let entry = DataForgeEntry::read_file_within(&file, 16, &|| false)?;

        assert_eq!(entry.path(), "exact.dcb");
        Ok(())
    }
}
