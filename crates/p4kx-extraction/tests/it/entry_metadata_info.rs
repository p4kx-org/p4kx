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
            "dos_timestamp": {"raw_date": 0, "raw_time": 0, "calendar": {"state": "invalid"}}
        }),
        "invalid source words and absent source fields are different states"
    );
    Ok(())
}
