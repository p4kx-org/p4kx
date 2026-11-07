// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use p4kx_archive::{MAX_METADATA_CHUNK, MetadataCost, MetadataReadError, MetadataSection};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::fixture::{self, Result};
use super::zip_fixture;

#[derive(Debug)]
struct BudgetMarker;

impl fmt::Display for BudgetMarker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("synthetic budget")
    }
}

impl Error for BudgetMarker {}

#[test]
fn root_and_nested_views_share_one_caller_budget_and_retain_its_original_cause() -> Result {
    let source = zip_fixture::zip(&[], &[], false)?;
    let opened = fixture::open(
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("room.socpak", source.bytes.as_slice()))
            .build(),
    )?;
    let entry = opened
        .archive
        .entries()
        .first()
        .ok_or("container Entry missing")?;
    let nested = opened.archive.open_container(entry)?;
    let cancelled = AtomicBool::new(false);
    let mut remaining = 4_u64;
    let mut calls = 0_u64;
    let mut charge = |cost: MetadataCost| {
        calls += cost.work_items();
        remaining = remaining
            .checked_sub(cost.read_bytes())
            .ok_or_else(|| io::Error::other(BudgetMarker))?;
        Ok(())
    };
    {
        let view = opened.archive.metadata_view();
        let span = view.span(MetadataSection::ZipEnd)?.ok_or("EOCD missing")?;
        view.access(&mut charge, &cancelled)
            .read_into(span, 0, &mut [0; 3])?;
    }
    let view = nested.metadata_view();
    let span = view.span(MetadataSection::ZipEnd)?.ok_or("EOCD missing")?;
    let failure = view
        .access(&mut charge, &cancelled)
        .read_into(span, 0, &mut [0; 2])
        .err()
        .ok_or("new view reset budget")?;
    let MetadataReadError::Budget(source) = failure else {
        return Err("wrong budget failure".into());
    };
    assert!(
        source
            .get_ref()
            .is_some_and(<dyn Error + Send + Sync>::is::<BudgetMarker>)
    );
    assert_eq!(calls, 2);
    assert_eq!(remaining, 1);
    Ok(())
}

#[test]
fn span_source_entry_identity_bounds_and_chunk_size_are_enforced() -> Result {
    let source = zip_fixture::zip(&[], &[], false)?;
    let first = fixture::open(&source.bytes)?;
    let second = fixture::open(&source.bytes)?;
    let first_view = first.archive.metadata_view();
    let second_view = second.archive.metadata_view();
    let span = first_view
        .span(MetadataSection::ZipEnd)?
        .ok_or("EOCD missing")?;
    let mut calls = 0;
    let mut charge = |_| {
        calls += 1;
        Ok(())
    };
    let cancelled = AtomicBool::new(false);
    let mut access = second_view.access(&mut charge, &cancelled);
    assert!(matches!(
        access.read_into(span, 0, &mut [0]),
        Err(MetadataReadError::ForeignSource)
    ));
    assert!(matches!(
        second_view.entry_record(first.archive.entries().first().ok_or("Entry missing")?),
        Err(MetadataReadError::ForeignEntry)
    ));
    let span = second_view
        .span(MetadataSection::ZipEnd)?
        .ok_or("EOCD missing")?;
    assert!(matches!(
        span.subspan(u64::MAX, 1),
        Err(MetadataReadError::OutsideSpan)
    ));
    assert!(matches!(
        access.read_into(span, 22, &mut [0]),
        Err(MetadataReadError::OutsideSpan)
    ));
    assert!(matches!(
        access.read_into(span, 0, &mut vec![0; MAX_METADATA_CHUNK + 1]),
        Err(MetadataReadError::ChunkLimit { .. })
    ));
    assert_eq!(calls, 0);
    Ok(())
}

#[test]
fn actual_cancellation_is_checked_before_budget_and_after_callback() -> Result {
    let source = zip_fixture::zip(&[], &[], false)?;
    let opened = fixture::open(&source.bytes)?;
    let view = opened.archive.metadata_view();
    let span = view.span(MetadataSection::ZipEnd)?.ok_or("EOCD missing")?;
    let cancelled = AtomicBool::new(true);
    let mut calls = 0;
    let mut charge = |_| {
        calls += 1;
        Ok(())
    };
    assert!(matches!(
        view.access(&mut charge, &cancelled)
            .read_into(span, 0, &mut [0]),
        Err(MetadataReadError::Cancelled)
    ));
    assert_eq!(calls, 0);
    cancelled.store(false, Ordering::Relaxed);
    let mut charge = |_| {
        cancelled.store(true, Ordering::Relaxed);
        Ok(())
    };
    assert!(matches!(
        view.access(&mut charge, &cancelled)
            .read_into(span, 0, &mut [0]),
        Err(MetadataReadError::Cancelled)
    ));
    Ok(())
}

#[test]
fn nested_stored_and_deflated_views_use_their_own_source_and_keep_omitted_folder_records() -> Result
{
    let inner = zip_fixture::zip(&[], &[], false)?;
    for entry in [
        TestEntry::stored("room.socpak", inner.bytes.as_slice()),
        TestEntry::deflated("room.socpak", inner.bytes.as_slice()),
    ] {
        let outer = fixture::open(&ArchiveBuilder::new().entry(entry).build())?;
        let entry = outer
            .archive
            .entries()
            .first()
            .ok_or("container Entry missing")?;
        let nested = outer.archive.open_container(entry)?;
        assert_eq!(nested.file_len(), outer.archive.file_len());
        assert_eq!(
            nested.metadata().source_len(),
            u64::try_from(inner.bytes.len())?
        );
        assert_ne!(nested.metadata().source_len(), nested.file_len());
        assert_eq!(nested.entries().len(), 1);
        let view = nested.metadata_view();
        let mut charge = |_| Ok(());
        let cancelled = AtomicBool::new(false);
        let mut access = view.access(&mut charge, &cancelled);
        let mut records = view.records()?;
        let folder = records
            .next_record(&mut access)?
            .ok_or("original folder record missing")?;
        assert_eq!(
            (
                folder.original_directory_ordinal(),
                folder.admitted_entry_index()
            ),
            (Some(0), None)
        );
        let file = records
            .next_record(&mut access)?
            .ok_or("original file record missing")?;
        assert_eq!(
            (
                file.original_directory_ordinal(),
                file.admitted_entry_index()
            ),
            (Some(1), Some(0))
        );
        assert_eq!(
            file.raw().range().offset(),
            u64::try_from(inner.records[1])?
        );
        assert!(records.next_record(&mut access)?.is_none());
        assert!(matches!(
            file.raw().subspan(u64::try_from(inner.bytes.len())?, 1),
            Err(MetadataReadError::OutsideSpan)
        ));
        assert_foreign_archive_rejected(&outer.archive, &mut access)?;
    }
    Ok(())
}

fn assert_foreign_archive_rejected<'source>(
    archive: &'source p4kx_archive::Archive,
    access: &mut p4kx_archive::MetadataAccess<'source, '_>,
) -> Result {
    let foreign = archive
        .metadata_view()
        .span(MetadataSection::ZipEnd)?
        .ok_or("outer EOCD missing")?;
    assert!(
        matches!(
            access.read_into(foreign, 0, &mut [0]),
            Err(MetadataReadError::ForeignSource)
        ),
        "a nested view cannot read its outer Archive's span"
    );
    Ok(())
}

#[test]
fn nested_v2_trie_validation_uses_decoded_source_length_in_both_container_routes() -> Result {
    let mut inner = super::v2_fixture::v2(1)?;
    let outside = u64::try_from(inner.bytes.len())? + 1;
    fixture::put(&mut inner.bytes, inner.trailer + 80, &outside.to_le_bytes())?;
    for entry in [
        TestEntry::stored("room.socpak", inner.bytes.as_slice()),
        TestEntry::deflated("room.socpak", inner.bytes.as_slice()),
    ] {
        let outer = fixture::open(&ArchiveBuilder::new().entry(entry).build())?;
        let nested = outer.archive.open_container(
            outer
                .archive
                .entries()
                .first()
                .ok_or("container Entry missing")?,
        )?;
        let view = nested.metadata_view();
        assert_eq!(view.source_len(), u64::try_from(inner.bytes.len())?);
        assert_eq!(
            view.metadata().status(),
            p4kx_archive::ArchiveMetadataStatus::Malformed
        );
        let region = view
            .region(MetadataSection::V2Trie)?
            .ok_or("trie declaration lost")?;
        assert_eq!(region.offset(), outside);
        assert_eq!(
            region.status(),
            p4kx_archive::MetadataRegionStatus::OutsideSource
        );
        assert!(region.available().is_none());
        assert!(matches!(
            view.span(MetadataSection::V2Trie),
            Err(MetadataReadError::MalformedRecord { .. })
        ));
    }
    Ok(())
}
