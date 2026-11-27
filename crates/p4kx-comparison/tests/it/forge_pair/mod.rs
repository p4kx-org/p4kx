// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Two Archives holding synthetic DataForges, compared on the Record level.

use std::error::Error;
use std::path::Path;

use p4kx_comparison::{
    Archive, CompareOptions, Comparison, DataForgeEntry, DataForges, Levels,
    compare_with_dataforge_entries,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};

/// The old and the new Archive, each with a `Data\Game2.dcb`, and their DataForge Entries.
pub(crate) struct ForgePair {
    _dir: tempfile::TempDir,
    pub(crate) old: Archive,
    pub(crate) new: Archive,
    pub(crate) old_entry: DataForgeEntry,
    pub(crate) new_entry: DataForgeEntry,
}

fn write(dir: &Path, channel: &str, forge: &DataForgeBuilder) -> Result<Archive, Box<dyn Error>> {
    let folder = dir.join(channel);
    std::fs::create_dir_all(&folder)?;
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", forge.build()))
        .write_to(&path)?;
    Ok(Archive::open_with_key(
        path,
        Some(p4kx_testkit::test_key()),
    )?)
}

impl ForgePair {
    /// Writes both Archives into a new temporary folder.
    pub(crate) fn new(
        old: &DataForgeBuilder,
        new: &DataForgeBuilder,
    ) -> Result<Self, Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let old = write(dir.path(), "LIVE", old)?;
        let new = write(dir.path(), "PTU", new)?;
        let old_entry = DataForgeEntry::read(&old)?;
        let new_entry = DataForgeEntry::read(&new)?;
        Ok(Self {
            _dir: dir,
            old,
            new,
            old_entry,
            new_entry,
        })
    }

    /// Compares the Records of both DataForges with `options`, every level when `levels` says so.
    pub(crate) fn compare_with<R>(
        &self,
        options: &CompareOptions<'_>,
        check: impl FnOnce(&Comparison<'_>) -> R,
    ) -> Result<R, Box<dyn Error>> {
        let (old, new) = (self.old_entry.open()?, self.new_entry.open()?);
        let comparison = compare_with_dataforge_entries(
            &self.old,
            &self.new,
            DataForges {
                old: &old,
                new: &new,
            },
            [&self.old_entry, &self.new_entry],
            options,
        )?;
        Ok(check(&comparison))
    }

    /// Compares only the Records of both DataForges.
    pub(crate) fn compare_records<R>(
        &self,
        check: impl FnOnce(&Comparison<'_>) -> R,
    ) -> Result<R, Box<dyn Error>> {
        self.compare_with(
            &CompareOptions {
                levels: Levels::RECORDS,
                ..CompareOptions::default()
            },
            check,
        )
    }
}
