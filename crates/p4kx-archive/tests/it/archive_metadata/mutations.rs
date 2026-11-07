// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Seek, SeekFrom, Write};
use std::sync::atomic::AtomicBool;

use p4kx_archive::{
    ArchiveMetadataStatus, MetadataExtraStatus, MetadataReadError, MetadataRegionStatus,
    MetadataSection,
};

use super::fixture::{self, Result};
use super::zip_fixture;

#[test]
fn malformed_comment_retains_available_bytes_without_claiming_following_padding() -> Result {
    let mut source = zip_fixture::zip(&[], &[], false)?;
    fixture::put(&mut source.bytes, source.end + 20, &u16::MAX.to_le_bytes())?;
    let opened = fixture::open(&source.bytes)?;
    let view = opened.archive.metadata_view();
    let region = view
        .region(MetadataSection::ZipComment)?
        .ok_or("comment declaration missing")?;
    assert_eq!(region.declared_len(), u64::from(u16::MAX));
    assert_eq!(region.status(), MetadataRegionStatus::Truncated);
    assert_eq!(view.metadata().status(), ArchiveMetadataStatus::Malformed);
    let available = view
        .available_span(MetadataSection::ZipComment)?
        .ok_or("available comment prefix lost")?;
    assert_eq!(available.status(), MetadataRegionStatus::Truncated);
    assert_eq!(
        fixture::read(&opened.archive, available.span())?,
        b"opaque\0\xffcommentPAD"
    );
    assert!(matches!(
        view.span(MetadataSection::ZipComment),
        Err(MetadataReadError::MalformedRecord { .. })
    ));
    assert!(matches!(
        view.span(MetadataSection::ZipFollowing),
        Err(MetadataReadError::MalformedSection)
    ));
    Ok(())
}

#[test]
fn unreadable_metadata_keeps_its_original_source_io_error() -> Result {
    let source = zip_fixture::zip(&[], &[], false)?;
    let opened = fixture::open(&source.bytes)?;
    let span = opened
        .archive
        .metadata_view()
        .span(MetadataSection::ZipEnd)?
        .ok_or("EOCD missing")?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(opened.archive.path())?
        .set_len(0)?;
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    let failure = opened
        .archive
        .metadata_view()
        .access(&mut charge, &cancelled)
        .read_into(span, 0, &mut [0; 22])
        .err()
        .ok_or("truncated source read succeeded")?;
    let MetadataReadError::Read { source, .. } = failure else {
        return Err("original source failure replaced".into());
    };
    assert_eq!(source.kind(), std::io::ErrorKind::UnexpectedEof);
    Ok(())
}

#[test]
fn changed_extra_framing_retains_partial_raw_bytes_and_stops_further_iteration() -> Result {
    for (new_len, status) in [
        (2_u16, MetadataExtraStatus::LengthBeforeHeader),
        (9, MetadataExtraStatus::TruncatedBody),
    ] {
        let source = zip_fixture::zip(&[0xbc, 0xfa, 5, 0, 0x7f], &[], false)?;
        let opened = fixture::open(&source.bytes)?;
        let view = opened.archive.metadata_view();
        let mut charge = |_| Ok(());
        let cancelled = AtomicBool::new(false);
        let mut access = view.access(&mut charge, &cancelled);
        let mut records = view.records()?;
        records.next_record(&mut access)?.ok_or("folder missing")?;
        let record = records.next_record(&mut access)?.ok_or("file missing")?;
        let extra = record.zip_extra().ok_or("extra extent missing")?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(opened.archive.path())?;
        file.seek(SeekFrom::Start(extra.range().offset() + 2))?;
        file.write_all(&new_len.to_le_bytes())?;
        file.flush()?;
        let mut fields = record.extras().ok_or("extra cursor missing")?;
        let field = fields
            .next_field(&mut access)?
            .ok_or("malformed field lost")?;
        assert_eq!(
            (field.id(), field.declared_total_len(), field.status()),
            (Some(0xfabc), Some(new_len), status)
        );
        assert_eq!(field.raw().range().len(), 5);
        assert!(matches!(
            fields.next_field(&mut access),
            Err(MetadataReadError::FailedIteration)
        ));
    }
    Ok(())
}

#[test]
fn a_short_extra_header_keeps_its_available_id_and_bytes() -> Result {
    let source = zip_fixture::zip(&[0xbc, 0xfa, 5, 0, 0x7f], &[], false)?;
    let opened = fixture::open(&source.bytes)?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(opened.archive.path())?;
    file.seek(SeekFrom::Start(u64::try_from(source.records[1] + 30)?))?;
    file.write_all(&3_u16.to_le_bytes())?;
    file.flush()?;
    let view = opened.archive.metadata_view();
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancelled);
    let mut records = view.records()?;
    records.next_record(&mut access)?.ok_or("folder missing")?;
    let record = records.next_record(&mut access)?.ok_or("file missing")?;
    let mut fields = record.extras().ok_or("extra cursor missing")?;
    let field = fields.next_field(&mut access)?.ok_or("short field lost")?;
    assert_eq!(
        (field.id(), field.declared_total_len(), field.status()),
        (Some(0xfabc), None, MetadataExtraStatus::ShortHeader)
    );
    assert_eq!(
        fixture::read(&opened.archive, field.raw())?,
        [0xbc, 0xfa, 5]
    );
    assert!(field.body().is_none());
    assert!(matches!(
        fields.next_field(&mut access),
        Err(MetadataReadError::FailedIteration)
    ));
    Ok(())
}

#[test]
fn overflowing_zip64_extension_keeps_the_declaration_and_available_prefix() -> Result {
    let mut source = zip_fixture::zip(&[], &[], true)?;
    let offset = source.zip64.ok_or("fixture ZIP64 missing")?;
    fixture::put(&mut source.bytes, offset + 4, &u64::MAX.to_le_bytes())?;
    let opened = fixture::open(&source.bytes)?;
    let view = opened.archive.metadata_view();
    let region = view
        .region(MetadataSection::Zip64Extension)?
        .ok_or("extension declaration lost")?;
    assert_eq!(region.declared_len(), u64::MAX - 44);
    assert_eq!(region.status(), MetadataRegionStatus::Overflow);
    assert_eq!(view.metadata().status(), ArchiveMetadataStatus::Malformed);
    assert!(matches!(
        view.span(MetadataSection::Zip64Extension),
        Err(MetadataReadError::MalformedRecord { .. })
    ));
    let available = view
        .available_span(MetadataSection::Zip64Extension)?
        .ok_or("extension prefix lost")?;
    assert_eq!(available.status(), MetadataRegionStatus::Overflow);
    assert_eq!(
        fixture::read(&opened.archive, available.span())?,
        source
            .bytes
            .get(offset + 56..)
            .ok_or("fixture prefix bounds")?
    );
    Ok(())
}

#[test]
fn declared_record_count_never_claims_unparsed_directory_tail_is_interpreted() -> Result {
    let mut source = zip_fixture::zip(&[], &[], false)?;
    source.bytes.splice(source.end..source.end, [0xab, 0xcd]);
    let size = u32::try_from(source.directory_len + 2)?;
    fixture::put(&mut source.bytes, source.end + 2 + 12, &size.to_le_bytes())?;
    let opened = fixture::open(&source.bytes)?;
    let view = opened.archive.metadata_view();
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancelled);
    let mut records = view.records()?;
    records
        .next_record(&mut access)?
        .ok_or("first record missing")?;
    records
        .next_record(&mut access)?
        .ok_or("second record missing")?;
    assert!(matches!(
        records.next_record(&mut access),
        Err(MetadataReadError::UnparsedDirectoryTail { len: 2, .. })
    ));
    assert!(matches!(
        records.next_record(&mut access),
        Err(MetadataReadError::FailedIteration)
    ));
    let raw = fixture::section(&opened.archive, MetadataSection::Directory)?;
    assert!(raw.ends_with(&[0xab, 0xcd]));
    Ok(())
}
