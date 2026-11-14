// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, EntryMetadataInfo};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::json;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

fn fixture() -> Result<(TempDir, Archive), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("synthetic.p4k");
    let mut bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("first.txt", b"a"))
        .entry(TestEntry::stored("second.txt", b"b"))
        .build();
    let records: Vec<_> = bytes
        .windows(4)
        .enumerate()
        .filter_map(|(offset, word)| (word == b"PK\x01\x02").then_some(offset))
        .collect();
    assert_eq!(
        records.len(),
        2,
        "the synthetic fixture has two directory records"
    );
    for (offset, (date, time)) in records.into_iter().zip([(0_u16, 0_u16), (0x285d, 0xbf7d)]) {
        if date != 0 {
            for (at, word) in [
                (4, 0x03ff_u16),
                (6, 0x02aa),
                (8, 0xf00d),
                (34, 0x1234),
                (36, 0x9669),
            ] {
                bytes
                    .get_mut(offset + at..offset + at + 2)
                    .ok_or("fixed header word")?
                    .copy_from_slice(&word.to_le_bytes());
            }
            bytes
                .get_mut(offset + 38..offset + 42)
                .ok_or("external attributes")?
                .copy_from_slice(&0xf123_4567_u32.to_le_bytes());
        }
        bytes
            .get_mut(offset + 12..offset + 14)
            .ok_or("fixture time word")?
            .copy_from_slice(&time.to_le_bytes());
        bytes
            .get_mut(offset + 14..offset + 16)
            .ok_or("fixture date word")?
            .copy_from_slice(&date.to_le_bytes());
    }
    std::fs::write(&path, bytes)?;
    Ok((folder, Archive::open(path)?))
}

#[test]
fn shared_metadata_encodes_raw_words_calendar_and_exact_physical_owner() -> TestResult {
    let (_folder, archive) = fixture()?;
    let entry = archive.entries().get(1).ok_or("second physical Entry")?;
    let encoded = serde_json::to_value(EntryMetadataInfo::of(entry))?;
    assert_eq!(
        encoded,
        json!({
            "owner_entry_index": 1,
            "record_format": "zip_central_directory",
            "original_directory_ordinal": "1",
            "raw_compression_method": 0,
            "zip_header": {"raw_version_made_by": 0x03ff_u16, "raw_version_needed": 0x02aa_u16,
                "general_purpose_flags": 0xf00d_u16, "disk_start": 0x1234_u16,
                "internal_attributes": 0x9669_u16, "external_attributes": 0xf123_4567_u32},
            "raw_v2_encryption_word": null,
            "dos_timestamp": {
                "raw_date": 0x285d_u16,
                "raw_time": 0xbf7d_u16,
                "calendar": {
                    "state": "valid", "year": 2000, "month": 2, "day": 29,
                    "hour": 23, "minute": 59, "second": 58
                }
            }
        }),
        "the shared projection retains source words and independent calendar expectations without a timezone"
    );
    Ok(())
}

#[test]
fn invalid_zero_words_stay_present_and_are_not_replaced_with_a_calendar_default() -> TestResult {
    let (_folder, archive) = fixture()?;
    let entry = archive.entries().first().ok_or("first physical Entry")?;
    assert_eq!(
        serde_json::to_value(EntryMetadataInfo::of(entry))?,
        json!({
            "owner_entry_index": 0,
            "dos_timestamp": {"raw_date": 0, "raw_time": 0, "calendar": {"state": "invalid"}},
            "record_format": "zip_central_directory",
            "original_directory_ordinal": "0",
            "raw_compression_method": 0,
            "zip_header": {"raw_version_made_by": 45, "raw_version_needed": 45,
                "general_purpose_flags": 0, "disk_start": 65535,
                "internal_attributes": 0, "external_attributes": 0},
            "raw_v2_encryption_word": null
        }),
        "invalid source words and absent source fields are different states"
    );
    Ok(())
}

#[test]
fn all_v2_encryption_bits_and_absent_zip_fields_remain_distinct_from_zero() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("v2.p4k");
    for encryption in [0_u16, 1, u16::MAX] {
        let mut bytes = ArchiveBuilder::v2()
            .entry(TestEntry::stored("raw.bin", b"abc"))
            .build();
        let directory = 3 + 8;
        bytes
            .get_mut(directory..directory + 2)
            .ok_or("method word")?
            .copy_from_slice(&0xd00d_u16.to_le_bytes());
        bytes
            .get_mut(directory + 170..directory + 172)
            .ok_or("encryption word")?
            .copy_from_slice(&encryption.to_le_bytes());
        std::fs::write(&path, bytes)?;
        let archive = Archive::open(&path)?;
        let entry = archive.entries().first().ok_or("v2 Entry")?;
        assert_eq!(
            serde_json::to_value(EntryMetadataInfo::of(entry))?,
            json!({
                "owner_entry_index": 0,
                "dos_timestamp": {"raw_date": 0, "raw_time": 0, "calendar": {"state": "invalid"}},
                "record_format": "p4k_v2", "original_directory_ordinal": "0",
                "raw_compression_method": 0xd00d_u16, "zip_header": null,
                "raw_v2_encryption_word": encryption
            }),
            "the projection preserves raw values without changing their interpretation"
        );
    }
    Ok(())
}

#[test]
fn decimal_source_integers_retain_every_bit_above_javascript_precision() -> TestResult {
    for (source, text) in [
        (0, "0"),
        (9_007_199_254_740_993, "9007199254740993"),
        (u64::MAX, "18446744073709551615"),
    ] {
        let value = p4kx_extraction::DecimalU64::new(source);
        assert_eq!(value.get(), source, "source bits");
        assert_eq!(
            serde_json::to_value(value)?,
            json!(text),
            "decimal JSON string"
        );
    }
    Ok(())
}
