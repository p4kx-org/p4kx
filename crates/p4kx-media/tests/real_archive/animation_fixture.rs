// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, TestAnimation, TestChunkFile, TestContainer, TestDba, TestEntry, TestTrack,
};

use super::rows::{Family, Totals};
use super::{TestResult, require};

pub(super) struct Expected {
    pub(super) family: Family,
    entries: u64,
    clips: u64,
    keys: u64,
}

pub(super) const EXPECTED: [Expected; 3] = [
    Expected {
        family: Family::Caf,
        entries: 2,
        clips: 2,
        keys: 4,
    },
    Expected {
        family: Family::Dba,
        entries: 1,
        clips: 2,
        keys: 4,
    },
    Expected {
        family: Family::Aim,
        entries: 1,
        clips: 0,
        keys: 0,
    },
];

impl Expected {
    pub(super) fn check(&self, totals: &Totals) -> TestResult {
        require(totals.declared == self.keys, "original fixture key count")?;
        require(totals.decoded == self.keys, "decoded fixture key count")?;
        require(
            totals.complete
                && totals.entries == self.entries
                && totals.clips == self.clips
                && totals.partial == 0,
            "census reconciles every synthetic physical source and clip",
        )
    }
}

pub(super) fn complete(path: &Path) -> TestResult<Archive> {
    let clip = TestAnimation::new("duplicate.caf", 30)
        .track(TestTrack::new(7).positions(&[(1, [0.0; 3]), (7, [1.0, 2.0, 3.0])]));
    let caf = clip.caf();
    let dba = TestDba::new()
        .animation(clip.clone())
        .animation(clip)
        .bytes();
    let nested = TestContainer::new()
        .entry(TestEntry::stored("duplicate.caf", caf.as_slice()))
        .entry(TestEntry::stored("pair.dba", dba))
        .build();
    let aim = TestChunkFile::ivo()
        .chunk(0x0BAD_BEEF, 0x900, b"opaque")
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("duplicate.caf", caf))
        .entry(TestEntry::stored("nested.pak", nested))
        .entry(TestEntry::stored("opaque.aim", aim))
        .write_to(path)?;
    Ok(Archive::open(path)?)
}
