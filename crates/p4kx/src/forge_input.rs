// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use anyhow::{Result, ensure};
use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeLoadError, DataForgeOrigin, DataForgeSource, SourceInput,
    is_dataforge_file,
};

/// The DataForge a `p4kx dcb` command reads, from an Archive or from a DataForge file on disk,
/// whichever the path given is.
pub(crate) struct ForgeInput {
    path: PathBuf,
    archive: Option<Archive>,
    entry: DataForgeEntry,
}

impl ForgeInput {
    /// Reads the DataForge at `path`: the file itself when its content is a DataForge header,
    /// otherwise the DataForge Entry of the Archive at `path`.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        crate::guard::on_archive_open(path);
        let sniffed = is_dataforge_file(path).map_err(|source| DataForgeLoadError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let archive = if sniffed {
            None
        } else {
            Some(crate::key::open(path)?)
        };
        let source = archive
            .as_ref()
            .map_or(DataForgeSource::File(path), DataForgeSource::Archive);
        let entry = source.read()?;
        Ok(Self {
            path: path.to_path_buf(),
            archive,
            entry,
        })
    }

    /// Returns the path that was opened.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the Archive, or `None` when a DataForge file was opened.
    pub(crate) const fn archive(&self) -> Option<&Archive> {
        self.archive.as_ref()
    }

    /// Returns the DataForge that was read.
    pub(crate) const fn entry(&self) -> &DataForgeEntry {
        &self.entry
    }

    /// Returns the physical input that supplied the admitted DataForge bytes.
    pub(crate) fn source_input(&self) -> Result<SourceInput<'_>> {
        let original = self.entry.origin();
        match original {
            DataForgeOrigin::Archive(origin) => {
                ensure!(
                    self.archive()
                        .is_some_and(|archive| original.matches_archive(archive)),
                    "foreign DataForge Archive Entry"
                );
                Ok(SourceInput::Archive(origin))
            }
            DataForgeOrigin::File(origin) => Ok(SourceInput::File(origin)),
            _ => Ok(SourceInput::Unknown),
        }
    }
}
