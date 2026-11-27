// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, Exclusion, Filter,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{ShaderIndex, ShaderLookup};
use crate::artifact::StreamObserver;
use crate::checkpoint::Checkpoint;
use crate::input_budget::InputBudget;
use crate::origin_test_support::TestResult;
use crate::source_selection::SourceSelection;
use crate::{ByteScope, Limits, MediaWriter, hashing};

const UNRELATED: u32 = 128;

#[test]
fn shader_lookup_work_ignores_unrelated_names_and_unused_duplicates() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = fixture(folder.path())?;
    let cancel = CancelToken::new();
    let watch = ArchiveWatch::new(&archive);
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let checkpoint = Checkpoint {
        cancel: &cancel,
        watch: &watch,
        protection: &protection,
    };
    let limits = Limits {
        reference_candidates: 1,
        ..Limits::default()
    };
    let (root, size) = hashing::file(
        archive.path(),
        ByteScope::RootArchiveFile,
        limits.root_bytes,
        &checkpoint,
    )?;
    let budget = InputBudget::default();
    let include = Filter::parse("*")?;
    let exclude = Exclusion::default();
    let lookup = ShaderLookup::new(
        &archive,
        &root,
        SourceSelection {
            include: &include,
            exclude: &exclude,
        },
        &budget,
    );
    let mut writer = MediaWriter::observed(
        folder.path(),
        Vec::new(),
        limits,
        checkpoint,
        false,
        &Discard,
    )?;
    writer.origins.observe_root(&archive, size)?;
    let index = ShaderIndex::build(&lookup, &mut writer)?;
    check_work(&index, &lookup, &writer)
}

fn check_work(
    index: &ShaderIndex,
    lookup: &ShaderLookup<'_>,
    writer: &MediaWriter<'_>,
) -> TestResult {
    let budget = lookup.budget;
    let before = budget.work_count()?;
    let query = "EXPECTED";
    let matches = index.matching(query, lookup, writer)?;
    assert_eq!(
        matches,
        [0, usize::try_from(UNRELATED)? + 1],
        "the bounded physical prefix"
    );
    assert_eq!(
        budget.work_count()? - before,
        u64::try_from(query.len() + 1 + matches.len())?,
        "charge only one key and the admitted prefix"
    );

    let before = budget.work_count()?;
    let query = "missing";
    assert!(
        index.matching(query, lookup, writer)?.is_empty(),
        "missing remains an empty candidate list"
    );
    assert_eq!(
        budget.work_count()? - before,
        u64::try_from(query.len() + 1)?,
        "a miss only charges its own key"
    );
    Ok(())
}

fn fixture(root: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = root.join("Data.p4k");
    let mut archive = ArchiveBuilder::new().entry(TestEntry::stored("Data/expected.ext", b""));
    for index in 0..UNRELATED {
        archive = archive.entry(TestEntry::stored(format!("Data/extra-{index}.ext"), b""));
    }
    archive
        .entry(TestEntry::stored("Data/Expected.ext", b""))
        .entry(TestEntry::stored("Data/ExPeCtEd.ext", b""))
        .write_to(&path)?;
    Ok(Archive::open(&path)?)
}

struct Discard;

impl StreamObserver for Discard {
    fn write(&self, _relative: &str, _bytes: &[u8]) -> io::Result<()> {
        Ok(())
    }
}
