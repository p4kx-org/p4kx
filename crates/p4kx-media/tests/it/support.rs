// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_media::{ExportOutcome, Index, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn archive(root: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/same.bin", b"first"))
        .entry(TestEntry::zstd("Data/same.bin", b"second"))
        .entry(TestEntry::stored("Data/empty.txt", b""))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

pub(super) fn published(outcome: ExportOutcome) -> Result<Index, Box<dyn std::error::Error>> {
    if let ExportOutcome::Published { index, .. } = outcome {
        Ok(*index)
    } else {
        Err("expected a complete published generation".into())
    }
}

pub(super) fn records(
    output: &Path,
    index: &Index,
    name: &str,
) -> Result<Vec<MediaRecord>, Box<dyn std::error::Error>> {
    fs::read_to_string(output.join(&index.artifact_base).join(name))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
