// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::AtomicBool;

use p4kx_archive::{
    ArchiveLayoutKind, ArchiveMetadataStatus, MetadataReadError, MetadataRegionStatus,
    MetadataReservedWord, MetadataSection, MetadataTableValue,
};

use super::fixture::{self, Result};
use super::v2_fixture;

#[test]
fn v2_base_extension_names_trie_and_both_distinct_trailer_slots_are_exact() -> Result {
    for base in 0..=2 {
        let source = v2_fixture::v2(base)?;
        let opened = fixture::open(&source.bytes)?;
        let archive = &opened.archive;
        let value = archive
            .metadata()
            .v2_directory()
            .ok_or("v2 declarations missing")?;
        assert_eq!(archive.metadata().layout(), ArchiveLayoutKind::P4kV2);
        assert_eq!(
            archive.metadata().status(),
            ArchiveMetadataStatus::Available
        );
        assert_eq!(
            (value.base_count(), value.extension_count()),
            (u64::try_from(base)?, u64::try_from(2 - base)?)
        );
        assert_eq!(value.directory_offset(), u64::try_from(source.directory)?);
        assert_eq!(value.names_offset(), u64::try_from(source.names)?);
        assert_eq!(value.trie_offset(), u64::try_from(source.trie)?);
        assert_eq!(
            (
                value.trie_size(),
                value.sector_size(),
                value.flags(),
                value.layout_version(),
                value.magic()
            ),
            (4, 4096, 0, 2, 0x696a_694a)
        );
        let names = fixture::section(archive, MetadataSection::V2BaseNames)?;
        let extension = fixture::section(archive, MetadataSection::V2ExtensionNames)?;
        assert_eq!([names, extension].concat(), b"Data/\0Data/a.bin\0");
        assert_eq!(
            fixture::section(archive, MetadataSection::V2BaseDirectory)?.len(),
            base * 204
        );
        assert_eq!(
            fixture::section(archive, MetadataSection::V2ExtensionDirectory)?.len(),
            (2 - base) * 204
        );
        assert_eq!(
            fixture::section(archive, MetadataSection::V2Trie)?,
            [0xde, 0xad, 0xbe, 0xef]
        );
        assert_eq!(
            fixture::section(archive, MetadataSection::V2OpaqueFirst)?,
            [0x19; 32]
        );
        assert_eq!(
            fixture::section(archive, MetadataSection::V2OpaqueSecond)?,
            [0xe1; 32]
        );
    }
    Ok(())
}

#[test]
fn signature_and_all_zero_stored_digest_are_present_without_cloning_at_open() -> Result {
    let source = v2_fixture::v2(1)?;
    let opened = fixture::open(&source.bytes)?;
    let view = opened.archive.metadata_view();
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    let mut access = view.access(&mut charge, &cancelled);
    let mut records = view.records()?;
    for ordinal in 0..2 {
        let record = records
            .next_record(&mut access)?
            .ok_or("source record missing")?;
        assert_eq!(record.original_directory_ordinal(), Some(ordinal));
        assert_eq!(
            record.raw().range().offset(),
            u64::try_from(source.directory)? + ordinal * 204
        );
        assert_eq!(
            fixture::read(
                &opened.archive,
                record.v2_signature()?.ok_or("signature missing")?
            )?,
            vec![0x80 + u8::try_from(ordinal)?; 128]
        );
        let expected = if ordinal == 0 { 0 } else { 0x7d };
        assert_eq!(
            fixture::read(
                &opened.archive,
                record.v2_stored_digest()?.ok_or("stored digest missing")?
            )?,
            vec![expected; 32]
        );
        assert!(record.zip_extra().is_none());
    }
    assert!(records.next_record(&mut access)?.is_none());
    assert_eq!(
        opened
            .archive
            .entries()
            .first()
            .ok_or("first Entry missing")?
            .sha256(),
        None
    );
    let mut rows = view
        .installation_records()?
        .ok_or("progress table missing")?;
    let MetadataTableValue::Installation(value) = rows
        .next_record(&mut access)?
        .ok_or("progress missing")?
        .value()
    else {
        return Err("wrong table kind".into());
    };
    assert_eq!(
        (value.written(), value.reserved_status()),
        (0, MetadataReservedWord::NotPresent)
    );
    Ok(())
}

#[test]
fn zero_digest_and_trie_words_remain_present_with_an_explicit_empty_extent() -> Result {
    let mut source = v2_fixture::v2(2)?;
    fixture::put(&mut source.bytes, source.trailer + 80, &[0; 24])?;
    fixture::put(&mut source.bytes, source.trailer + 105, &[0; 64])?;
    let opened = fixture::open(&source.bytes)?;
    let value = opened
        .archive
        .metadata()
        .v2_directory()
        .ok_or("v2 metadata missing")?;
    assert_eq!(
        (value.trie_offset(), value.trie_size(), value.sector_size()),
        (0, 0, 0)
    );
    assert_eq!(
        fixture::section(&opened.archive, MetadataSection::V2Trie)?,
        Vec::<u8>::new()
    );
    assert_eq!(
        fixture::section(&opened.archive, MetadataSection::V2OpaqueFirst)?,
        [0; 32]
    );
    assert_eq!(
        fixture::section(&opened.archive, MetadataSection::V2OpaqueSecond)?,
        [0; 32]
    );
    Ok(())
}

#[test]
fn bad_trie_declarations_keep_raw_words_and_checked_prefixes_without_changing_open_policy() -> Result
{
    for (offset, len, status) in [
        (u64::MAX, 2, MetadataRegionStatus::Overflow),
        (2, u64::MAX, MetadataRegionStatus::Overflow),
        (10_000, 1, MetadataRegionStatus::OutsideSource),
    ] {
        let mut source = v2_fixture::v2(1)?;
        fixture::put(
            &mut source.bytes,
            source.trailer + 80,
            &offset.to_le_bytes(),
        )?;
        fixture::put(&mut source.bytes, source.trailer + 88, &len.to_le_bytes())?;
        let opened = fixture::open(&source.bytes)?;
        let view = opened.archive.metadata_view();
        let region = view
            .region(MetadataSection::V2Trie)?
            .ok_or("trie declaration missing")?;
        assert_eq!(
            (region.offset(), region.declared_len(), region.status()),
            (offset, len, status)
        );
        assert_eq!(view.metadata().status(), ArchiveMetadataStatus::Malformed);
        assert!(matches!(
            view.span(MetadataSection::V2Trie),
            Err(MetadataReadError::MalformedRecord { .. })
        ));
        assert_eq!(opened.archive.issue_count(), 0);
    }
    Ok(())
}

#[test]
fn incomplete_flag_and_truncated_fixed_layout_still_refuse_normal_open() -> Result {
    let mut source = v2_fixture::v2(1)?;
    fixture::put(&mut source.bytes, source.trailer + 104, &[1])?;
    assert!(fixture::open(&source.bytes).is_err());
    fixture::put(&mut source.bytes, source.trailer + 104, &[0])?;
    fixture::put(
        &mut source.bytes,
        source.trailer + 72,
        &u64::MAX.to_le_bytes(),
    )?;
    assert!(fixture::open(&source.bytes).is_err());
    source.bytes.truncate(source.trailer + 170);
    assert!(fixture::open(&source.bytes).is_err());
    Ok(())
}
