// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::DirectoryIndex;
use crate::checkpoint::Checkpoint;
use crate::lookup_work::LookupWork;
use crate::{Limits, MediaError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn actual_cancellation_stops_before_index_storage_and_keeps_the_original_category() -> TestResult {
    with_source(|archive, cancel, work, slots| {
        let index = DirectoryIndex::new(archive, slots);
        cancel.cancel();
        let failure = index.index(work).err().ok_or("cancelled index admitted")?;
        assert!(
            matches!(failure, MediaError::Cancelled),
            "actual job cancellation remains typed"
        );
        assert!(
            index.keys.get().is_none(),
            "cancelled work creates no successful inventory"
        );
        assert_eq!(slots.get(), 0, "cancelled index reserves no live slots");
        assert_eq!(
            work.current.get(),
            0,
            "cancelled work is not charged as completed work"
        );
        Ok(())
    })
}

#[test]
fn active_directory_keys_share_one_bound_and_release_their_own_slots() -> TestResult {
    with_source(|archive, _, work, slots| {
        let first = DirectoryIndex::new(archive, slots);
        first.index(work)?;
        assert_eq!(
            slots.get(),
            2,
            "a built directory owns exactly its Entry count"
        );
        first.index(work)?;
        assert_eq!(
            slots.get(),
            2,
            "reusing an index neither rebuilds nor doubles its reservation"
        );
        let second = DirectoryIndex::new(archive, slots);
        assert!(
            matches!(
                second.index(work),
                Err(MediaError::Limit {
                    budget: "sources",
                    limit: 3
                })
            ),
            "simultaneously active directories cannot exceed the shared key-storage bound"
        );
        drop(first);
        assert_eq!(
            slots.get(),
            0,
            "dropping one directory releases only its own keys"
        );
        second.index(work)?;
        assert_eq!(
            slots.get(),
            2,
            "a later directory can reuse the released capacity"
        );
        drop(second);
        assert_eq!(slots.get(), 0, "all directory leases leave no shared state");
        Ok(())
    })
}

fn with_source(
    run: impl FnOnce(&Archive, &CancelToken, &LookupWork<'_>, &Cell<u64>) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/a.mtl", b""))
        .entry(TestEntry::stored("Data/b.mtl", b""))
        .write_to(&source)?;
    let archive = Archive::open(&source)?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    let watch = ArchiveWatch::new(&archive);
    let current = Cell::new(0);
    let slots = Cell::new(0);
    let limits = Limits {
        sources: 3,
        ..Limits::default()
    };
    let work = LookupWork {
        checkpoint: Checkpoint {
            cancel: &cancel,
            watch: &watch,
            protection: &protection,
        },
        limits,
        current: &current,
        source_allowance: 3,
    };
    run(&archive, &cancel, &work, &slots)
}
