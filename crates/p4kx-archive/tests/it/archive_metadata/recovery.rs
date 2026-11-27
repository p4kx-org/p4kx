// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::AtomicBool;

use p4kx_archive::{ArchiveLayoutKind, ArchiveMetadataStatus, MetadataReadError, MetadataSection};

use super::fixture::{self, Result};
use super::zip_fixture;

#[test]
fn recovery_keeps_surviving_record_extents_without_inventing_a_global_layout_or_ordinal() -> Result
{
    let mut source = zip_fixture::zip(&[], &[], false)?;
    source.bytes.truncate(source.end);
    let opened = fixture::open_with(&source.bytes, true)?;
    let view = opened.archive.metadata_view();
    assert_eq!(
        view.metadata().layout(),
        ArchiveLayoutKind::RecoveredUnknown
    );
    assert_eq!(view.metadata().status(), ArchiveMetadataStatus::Recovered);
    assert!(view.metadata().zip_directory().is_none());
    assert!(matches!(
        view.span(MetadataSection::Directory),
        Err(MetadataReadError::UnavailableLayout)
    ));
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancelled);
    let mut records = view.records()?;
    for entry in opened.archive.entries() {
        let record = records
            .next_record(&mut access)?
            .ok_or("recovered record missing")?;
        assert_eq!(record.original_directory_ordinal(), None);
        assert_eq!(record.admitted_entry_index(), Some(entry.index()));
        assert_eq!(record.raw().range(), entry.metadata().record_range());
        let expected = source
            .bytes
            .get(
                usize::try_from(record.raw().range().offset())?
                    ..usize::try_from(record.raw().range().offset() + record.raw().range().len())?,
            )
            .ok_or("fixture record bounds")?;
        assert_eq!(fixture::read(&opened.archive, record.raw())?, expected);
    }
    assert!(records.next_record(&mut access)?.is_none());
    Ok(())
}

#[test]
fn recover_option_on_an_intact_archive_does_not_erase_proved_layouts_or_ordinals() -> Result {
    let source = zip_fixture::zip(&[], &[], false)?;
    let opened = fixture::open_with(&source.bytes, true)?;
    assert_eq!(opened.archive.metadata().layout(), ArchiveLayoutKind::Zip);
    assert_eq!(
        opened.archive.metadata().status(),
        ArchiveMetadataStatus::Available
    );
    assert_eq!(
        opened
            .archive
            .entries()
            .last()
            .ok_or("Entry missing")?
            .metadata()
            .original_directory_ordinal(),
        Some(1)
    );
    Ok(())
}
