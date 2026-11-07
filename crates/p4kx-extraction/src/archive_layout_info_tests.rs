// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::{Archive, ArchiveLayoutInfo};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const EXPECTED: &str = include_str!("../../p4kx-testkit/fixtures/archive-layout-v2-empty.json");

#[test]
fn cached_layout_info_matches_independent_v2_source_words() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("x", b""))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let actual = serde_json::to_value(ArchiveLayoutInfo::of(archive.metadata()))?;
    assert_eq!(
        actual,
        serde_json::from_str::<serde_json::Value>(EXPECTED)?,
        "independently authored v2 declarations"
    );
    assert!(
        actual.get("signatures").is_none(),
        "header summaries do not load per-Entry signatures"
    );
    Ok(())
}

#[test]
fn header_projection_needs_no_additional_source_io_or_lazy_row_validation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("x", b""))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(0)?;
    assert_eq!(
        serde_json::to_value(ArchiveLayoutInfo::of(archive.metadata()))?,
        serde_json::from_str::<serde_json::Value>(EXPECTED)?,
        "cached header availability is not a fresh file or lazy-row validation result"
    );
    Ok(())
}
