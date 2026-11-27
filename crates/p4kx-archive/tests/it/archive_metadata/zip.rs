// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::AtomicBool;

use p4kx_archive::{
    ArchiveLayoutKind, ArchiveMetadataStatus, MetadataExtraStatus, MetadataFreeStatus,
    MetadataProgressBound, MetadataReadError, MetadataReservedWord, MetadataSection,
    MetadataTableValue, MetadataTables,
};

use super::fixture::{self, Result};
use super::zip_fixture;

#[test]
fn classic_end_and_binary_comment_words_remain_source_relative() -> Result {
    let source = zip_fixture::zip(&[], &[], false)?;
    let opened = fixture::open(&source.bytes)?;
    let archive = &opened.archive;
    let zip = archive
        .metadata()
        .zip_directory()
        .ok_or("ZIP metadata missing")?;
    let end = zip.end();
    assert_eq!(archive.metadata().layout(), ArchiveLayoutKind::Zip);
    assert_eq!(
        archive.metadata().status(),
        ArchiveMetadataStatus::Available
    );
    assert_eq!(
        archive.metadata().source_len(),
        u64::try_from(source.bytes.len())?
    );
    assert_eq!(
        (
            end.disk(),
            end.directory_disk(),
            end.entries_on_disk(),
            end.entries()
        ),
        (7, 11, 1, 2)
    );
    assert_eq!(end.fixed().offset(), u64::try_from(source.end)?);
    assert_eq!(zip.records().offset(), u64::try_from(source.directory)?);
    assert_eq!(zip.records().len(), u64::try_from(source.directory_len)?);
    assert_eq!(
        fixture::section(archive, MetadataSection::ZipComment)?,
        b"opaque\0\xffcomment"
    );
    assert_eq!(
        fixture::section(archive, MetadataSection::ZipFollowing)?,
        b"PAD"
    );
    assert!(zip.zip64().is_none());
    assert!(zip.installation().is_none());
    assert!(archive.metadata_view().installation_records()?.is_none());
    Ok(())
}

#[test]
fn zip64_extension_locator_and_recognised_cig_words_are_retained() -> Result {
    let source = zip_fixture::zip(&[], &[], true)?;
    let opened = fixture::open(&source.bytes)?;
    let archive = &opened.archive;
    let zip = archive
        .metadata()
        .zip_directory()
        .ok_or("ZIP metadata missing")?;
    let end = zip.zip64().ok_or("ZIP64 metadata missing")?;
    let locator = zip.locator().ok_or("locator missing")?;
    let install = zip.installation().ok_or("CIG declaration missing")?;
    assert_eq!(archive.metadata().layout(), ArchiveLayoutKind::Zip64);
    assert_eq!(
        (
            end.declared_size(),
            end.version_made_by(),
            end.version_needed()
        ),
        (48, 0x031e, 45)
    );
    assert_eq!(
        (
            end.disk(),
            end.directory_disk(),
            end.entries_on_disk(),
            end.entries()
        ),
        (17, 19, 1, 2)
    );
    assert_eq!(
        end.fixed().offset(),
        u64::try_from(source.zip64.ok_or("fixture ZIP64 missing")?)?
    );
    assert_eq!(
        locator.range().offset(),
        u64::try_from(source.locator.ok_or("fixture locator missing")?)?
    );
    assert_eq!((locator.disk(), locator.disks()), (23, 29));
    assert_eq!(locator.target(), end.fixed().offset());
    assert_eq!(
        fixture::section(archive, MetadataSection::Zip64Extension)?,
        [0, 0xfe, 0xff, 0x7a]
    );
    assert_eq!((install.sector_size(), install.free_count()), (1024, 2));
    let MetadataTables::Located { progress, free } = install.tables() else {
        return Err("valid table range was lost".into());
    };
    assert_eq!((progress.len(), free.len()), (24, 32));
    assert_eq!(progress.offset() + 56, u64::try_from(source.directory)?);
    Ok(())
}

#[test]
fn lazy_installation_rows_keep_invalid_reserved_words_without_health_reclassification() -> Result {
    let source = zip_fixture::zip(&[], &[], true)?;
    let opened = fixture::open(&source.bytes)?;
    let view = opened.archive.metadata_view();
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancelled);
    let mut rows = view
        .installation_records()?
        .ok_or("progress table missing")?;
    let first = rows
        .next_record(&mut access)?
        .ok_or("first progress missing")?;
    assert_eq!(first.ordinal(), 0);
    let MetadataTableValue::Installation(first) = first.value() else {
        return Err("unexpected table value".into());
    };
    assert_eq!(
        (first.written(), first.reserved_status()),
        (0, MetadataReservedWord::Zero)
    );
    let row = rows
        .next_record(&mut access)?
        .ok_or("second progress missing")?;
    let MetadataTableValue::Installation(value) = row.value() else {
        return Err("unexpected table value".into());
    };
    assert_eq!(
        (value.written(), value.reserved(), value.bound()),
        (
            3,
            Some(0x1234_5678),
            MetadataProgressBound::WithinStoredSize
        )
    );
    assert_eq!(value.reserved_status(), MetadataReservedWord::Nonzero);
    assert!(rows.next_record(&mut access)?.is_none());
    assert_eq!(opened.archive.issue_count(), 0);
    Ok(())
}

#[test]
fn lazy_free_rows_keep_zero_and_overflow_words_without_health_reclassification() -> Result {
    let source = zip_fixture::zip(&[], &[], true)?;
    let opened = fixture::open(&source.bytes)?;
    let view = opened.archive.metadata_view();
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancelled);
    let mut rows = view.free_records()?.ok_or("free table missing")?;
    let MetadataTableValue::FreeRange(first) = rows
        .next_record(&mut access)?
        .ok_or("first free record missing")?
        .value()
    else {
        return Err("unexpected table value".into());
    };
    assert_eq!(first.status(), MetadataFreeStatus::ZeroSize);
    let MetadataTableValue::FreeRange(second) = rows
        .next_record(&mut access)?
        .ok_or("second free record missing")?
        .value()
    else {
        return Err("unexpected table value".into());
    };
    assert_eq!(
        (
            second.extent().offset(),
            second.extent().declared_len(),
            second.status()
        ),
        (u64::MAX, 8, MetadataFreeStatus::Overflow)
    );
    assert!(rows.next_record(&mut access)?.is_none());
    assert_eq!(opened.archive.issue_count(), 0);
    Ok(())
}

fn extra_fields() -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for (id, data) in [
        (0x5002_u16, vec![1, 0]),
        (0xfabc, vec![0xff]),
        (0x5003, vec![9; 32]),
        (0xfabc, vec![0]),
        (0x5002, vec![0, 0]),
        (0x5003, vec![0; 32]),
    ] {
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.extend_from_slice(&u16::try_from(data.len() + 4)?.to_le_bytes());
        bytes.extend_from_slice(&data);
    }
    Ok(bytes)
}

#[test]
fn ordered_duplicate_and_unknown_extras_preserve_total_length_framing_and_comments() -> Result {
    let extras = extra_fields()?;
    for zip64 in [false, true] {
        let source = zip_fixture::zip(&extras, b"\0\xffbinary", zip64)?;
        let opened = fixture::open(&source.bytes)?;
        let view = opened.archive.metadata_view();
        let mut charge = |_| Ok(());
        let cancelled = AtomicBool::new(false);
        let mut access = view.access(&mut charge, &cancelled);
        let mut records = view.records()?;
        records
            .next_record(&mut access)?
            .ok_or("folder record missing")?;
        let record = records
            .next_record(&mut access)?
            .ok_or("file record missing")?;
        assert_eq!(record.original_directory_ordinal(), Some(1));
        assert_eq!(
            fixture::read(
                &opened.archive,
                record.zip_name().ok_or("path bytes missing")?
            )?,
            b"Data/a.bin"
        );
        assert_eq!(
            fixture::read(
                &opened.archive,
                record.zip_extra().ok_or("extra bytes missing")?
            )?,
            extras
        );
        assert_eq!(
            fixture::read(
                &opened.archive,
                record.zip_comment().ok_or("comment bytes missing")?
            )?,
            b"\0\xffbinary"
        );
        assert_extra_order(record, &mut access)?;
        assert!(records.next_record(&mut access)?.is_none());
        let entry = opened
            .archive
            .entries()
            .last()
            .ok_or("file Entry missing")?;
        assert!(!entry.is_encrypted());
        assert_eq!(entry.sha256(), None);
    }
    Ok(())
}

fn assert_extra_order<'source>(
    record: p4kx_archive::MetadataRecord<'source>,
    access: &mut p4kx_archive::MetadataAccess<'source, '_>,
) -> Result {
    let mut fields = record.extras().ok_or("extra cursor missing")?;
    let mut actual = Vec::new();
    while let Some(field) = fields.next_field(access)? {
        assert_eq!(
            field.status(),
            MetadataExtraStatus::Complete,
            "each authored extra has its complete declared framing"
        );
        actual.push((field.id(), field.declared_total_len()));
    }
    assert_eq!(
        actual,
        [
            (Some(0x5002), Some(6)),
            (Some(0xfabc), Some(5)),
            (Some(0x5003), Some(36)),
            (Some(0xfabc), Some(5)),
            (Some(0x5002), Some(6)),
            (Some(0x5003), Some(36))
        ],
        "unknown and repeated fields retain their original order and total lengths"
    );
    Ok(())
}

#[test]
fn invalid_known_table_counts_and_zip64_sizes_are_never_silent_absence() -> Result {
    let mut source = zip_fixture::zip(&[], &[], true)?;
    fixture::put(&mut source.bytes, source.end + 30, &u64::MAX.to_le_bytes())?;
    let opened = fixture::open(&source.bytes)?;
    assert_eq!(
        opened.archive.metadata().status(),
        ArchiveMetadataStatus::Malformed
    );
    assert!(matches!(
        opened.archive.metadata_view().free_records(),
        Err(MetadataReadError::MalformedSection)
    ));
    let offset = source.zip64.ok_or("fixture ZIP64 missing")?;
    fixture::put(&mut source.bytes, source.end + 30, &2_u64.to_le_bytes())?;
    fixture::put(&mut source.bytes, offset + 4, &0_u64.to_le_bytes())?;
    let opened = fixture::open(&source.bytes)?;
    assert_eq!(
        opened
            .archive
            .metadata()
            .zip_directory()
            .and_then(|zip| zip.zip64())
            .map(|zip| zip.declared_size()),
        Some(0)
    );
    assert!(matches!(
        opened
            .archive
            .metadata_view()
            .span(MetadataSection::Zip64Extension),
        Err(MetadataReadError::MalformedSection)
    ));
    Ok(())
}
