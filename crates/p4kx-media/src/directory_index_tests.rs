// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::DirectoryIndex;
use crate::checkpoint::Checkpoint;
use crate::input_budget::InputBudget;
use crate::lookup_work::LookupWork;
use crate::{Limits, MediaError};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn actual_cancellation_stops_before_index_storage_and_keeps_the_original_category() -> TestResult {
    with_source(|archive, cancel, work, slots, _| {
        let index = DirectoryIndex::new(archive, slots);
        cancel.cancel();
        let failure = index.index(work).err().ok_or("cancelled index admitted")?;
        let family_failure = index
            .texture_members(work)
            .err()
            .ok_or("cancelled family index admitted")?;
        assert!(matches!(family_failure, MediaError::Cancelled));
        assert!(index.members.get().is_none());
        assert_eq!(index.registered_entries.get(), None);
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
    with_source(|archive, _, work, slots, _| {
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

#[test]
fn either_build_order_registers_one_directory_without_source_admission() -> TestResult {
    for members_first in [true, false] {
        with_source(|archive, _, work, slots, budget| {
            let index = DirectoryIndex::new(archive, slots);
            if members_first {
                index.texture_members(work)?;
                index.index(work)?;
            } else {
                index.index(work)?;
                index.texture_members(work)?;
            }
            assert_eq!(
                slots.get(),
                2,
                "two views share physical-directory registration"
            );
            assert_eq!(
                budget.sources.get(),
                0,
                "index cells are not source admissions"
            );
            assert_eq!(index.registered_entries.get(), Some(2));
            assert!(index.keys.get().is_some());
            assert!(index.members.get().is_some());
            let charged = work.current.get();
            index.index(work)?;
            index.texture_members(work)?;
            assert_eq!(
                work.current.get(),
                charged,
                "cached access does not rebuild"
            );
            drop(index);
            assert_eq!(slots.get(), 0);
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn nested_frames_bound_both_views_and_release_only_their_own_registration() -> TestResult {
    with_source(|archive, _, work, slots, budget| {
        let expanded = LookupWork {
            limits: Limits {
                sources: 4,
                ..work.limits
            },
            checkpoint: work.checkpoint,
            current: work.current,
            source_allowance: 4,
        };
        let outer = DirectoryIndex::new(archive, slots);
        outer.texture_members(&expanded)?;
        {
            let inner = DirectoryIndex::new(archive, slots);
            inner.index(&expanded)?;
            assert_eq!(slots.get(), 4);
            inner.texture_members(&expanded)?;
            outer.index(&expanded)?;
            assert_eq!(slots.get(), 4, "a second view is not another source frame");
            assert_eq!(budget.sources.get(), 0);
        }
        assert_eq!(slots.get(), 2);
        outer.texture_members(&expanded)?;
        drop(outer);
        assert_eq!(slots.get(), 0);
        Ok(())
    })
}

#[test]
fn a_pending_family_build_is_included_in_the_shared_source_bound() -> TestResult {
    with_source(|archive, _, work, slots, _| {
        let outer = DirectoryIndex::new(archive, slots);
        outer.index(work)?;
        let pending = DirectoryIndex::new(archive, slots);
        let charged = work.current.get();

        let failure = pending
            .texture_members(work)
            .err()
            .ok_or("pending frame admitted")?;

        assert!(matches!(
            failure,
            MediaError::Limit {
                budget: "sources",
                limit: 3
            }
        ));
        assert_eq!(
            work.current.get(),
            charged,
            "no directory control work is precharged"
        );
        assert!(pending.members.get().is_none());
        assert_eq!(pending.registered_entries.get(), None);
        assert_eq!(slots.get(), 2);
        Ok(())
    })
}

#[test]
fn insufficient_actual_source_allowance_never_publishes_an_inventory() -> TestResult {
    with_source(|archive, _, work, slots, budget| {
        let limited = LookupWork {
            source_allowance: 1,
            checkpoint: work.checkpoint,
            limits: work.limits,
            current: work.current,
        };
        let index = DirectoryIndex::new(archive, slots);
        assert!(matches!(
            index.texture_members(&limited),
            Err(MediaError::Limit {
                budget: "sources",
                limit: 3
            })
        ));
        assert!(index.members.get().is_none());
        assert_eq!(index.registered_entries.get(), None);
        assert_eq!(slots.get(), 0);
        assert_eq!(budget.sources.get(), 0);
        Ok(())
    })
}

#[test]
fn a_mid_build_work_failure_registers_neither_index_nor_complete_inventory() -> TestResult {
    with_source(|archive, _, work, slots, budget| {
        let limited = LookupWork {
            limits: Limits {
                metadata_work: 1,
                ..work.limits
            },
            checkpoint: work.checkpoint,
            current: work.current,
            source_allowance: work.source_allowance,
        };
        let index = DirectoryIndex::new(archive, slots);
        assert!(matches!(
            index.texture_members(&limited),
            Err(MediaError::Limit {
                budget: "metadata_work",
                limit: 1
            })
        ));
        assert!(index.keys.get().is_none());
        assert!(index.members.get().is_none());
        assert_eq!(index.registered_entries.get(), None);
        assert_eq!(slots.get(), 0);
        assert_eq!(budget.sources.get(), 0);
        assert!(matches!(
            index.inventory(),
            crate::CandidateInventory::Unavailable(crate::ReferenceUnavailable::TargetNotRead)
        ));
        index.texture_members(work)?;
        assert_eq!(
            slots.get(),
            2,
            "a later admitted retry builds the real index"
        );
        Ok(())
    })
}

#[test]
fn a_failed_second_view_keeps_only_the_first_view_registration() -> TestResult {
    with_source(|archive, _, work, slots, budget| {
        let index = DirectoryIndex::new(archive, slots);
        index.index(work)?;
        let limit = work.current.get() + 1;
        let limited = LookupWork {
            limits: Limits {
                metadata_work: limit,
                ..work.limits
            },
            checkpoint: work.checkpoint,
            current: work.current,
            source_allowance: work.source_allowance,
        };
        assert!(matches!(
            index.texture_members(&limited),
            Err(MediaError::Limit {
                budget: "metadata_work",
                ..
            })
        ));
        assert!(index.keys.get().is_some());
        assert!(index.members.get().is_none());
        assert_eq!(index.registered_entries.get(), Some(2));
        assert_eq!(slots.get(), 2);
        assert_eq!(budget.sources.get(), 0);
        index.texture_members(work)?;
        assert_eq!(slots.get(), 2);
        Ok(())
    })
}

#[test]
fn an_actual_source_change_prevents_a_family_index_or_registration() -> TestResult {
    use std::io::Write as _;

    with_source(|archive, _, work, slots, _| {
        std::fs::OpenOptions::new()
            .append(true)
            .open(archive.path())?
            .write_all(b"x")?;
        let index = DirectoryIndex::new(archive, slots);

        let failure = index
            .texture_members(work)
            .err()
            .ok_or("changed source admitted")?;

        assert!(matches!(failure, MediaError::Changed(_)));
        assert!(index.members.get().is_none());
        assert_eq!(index.registered_entries.get(), None);
        assert_eq!(slots.get(), 0);
        Ok(())
    })
}

fn with_source(
    run: impl FnOnce(&Archive, &CancelToken, &LookupWork<'_>, &Cell<u64>, &InputBudget) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/a.dds", b""))
        .entry(TestEntry::stored("Data/a.dds.1", b""))
        .write_to(&source)?;
    let archive = Archive::open(&source)?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    let watch = ArchiveWatch::new(&archive).with_interval(std::time::Duration::ZERO);
    let budget = InputBudget::default();
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
        current: &budget.work,
        source_allowance: 3,
    };
    run(&archive, &cancel, &work, &slots, &budget)
}
