// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read as _;

use p4kx_search::{self as search, EntryHit, Found, Needle, SearchOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::common::{self, Fixture};

pub(crate) const CUT: usize = 1_048_576;
pub(crate) const READ_EDGE: usize = 65_552;
pub(crate) const PATH: &str = "synthetic\\complete.txt";
pub(crate) type Checked<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) fn fixture(bytes: &[u8]) -> Checked<Fixture> {
    let builder = ArchiveBuilder::new().entry(TestEntry::stored(PATH, bytes.to_vec()));
    let fixture = common::fixture(&builder)?;
    let entry =
        p4kx_extraction::find_entry(&fixture.archive, PATH).ok_or("original synthetic Entry")?;
    let mut actual = Vec::new();
    fixture
        .archive
        .open_entry(entry)?
        .read_to_end(&mut actual)?;
    assert_eq!(
        actual, bytes,
        "the actual Archive stores the complete independent original input"
    );
    Ok(fixture)
}

pub(crate) fn run(
    fixture: &Fixture,
    text: &str,
    options: &SearchOptions,
) -> Checked<Found<EntryHit>> {
    search::search_entries(&fixture.archive, &Needle::new(text)?, options, &|| false)
        .ok_or_else(|| "complete source search was unexpectedly cancelled".into())
}

pub(crate) fn complete(found: &Found<EntryHit>, expected: usize) {
    assert_eq!(
        found.hits.len(),
        expected,
        "one hit per matching original line"
    );
    assert_eq!(
        found.stats.entries_scanned, 1,
        "the one original Entry was scanned"
    );
    assert_eq!(
        found.stats.entries_skipped(),
        0,
        "no source bytes were refused or classified binary"
    );
    assert!(!found.has_more, "there is no global result-limit omission");
    assert!(
        found.hits.iter().all(|hit| hit.path == PATH),
        "every hit belongs to the original Entry"
    );
}

pub(crate) fn hit(found: &Found<EntryHit>, index: usize) -> Checked<&EntryHit> {
    found
        .hits
        .get(index)
        .ok_or_else(|| "expected original matching line was omitted".into())
}
