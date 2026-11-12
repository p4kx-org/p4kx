// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reading the data of Entries from an offset.

use std::error::Error;
use std::io::Read;

use p4kx_archive::{Archive, ArchiveError, MAX_DECODED_SKIP};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

type TestResult = Result<(), Box<dyn Error>>;

fn data() -> Vec<u8> {
    (0..700_u32)
        .map(|index| u8::try_from(index * 7 % 251).unwrap_or_default())
        .collect()
}

fn archive(entries: Vec<TestEntry>) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    Ok((dir, archive))
}

fn read_from(archive: &Archive, index: usize, offset: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let entry = archive.entries().get(index).ok_or("Entry missing")?;
    let mut bytes = Vec::new();
    archive
        .open_entry_at(entry, offset)?
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[test]
fn every_kind_of_entry_reads_from_any_offset_to_its_end() -> TestResult {
    let data = data();
    let (_dir, archive) = archive(vec![
        TestEntry::stored("Data\\stored.bin", data.clone()),
        TestEntry::stored("Data\\encrypted.bin", data.clone()).encrypted(),
        TestEntry::deflated("Data\\deflated.bin", data.clone()),
        TestEntry::zstd("Data\\zstd.bin", data.clone()).encrypted(),
    ])?;
    for index in 0..4 {
        for offset in [0, 1, 15, 16, 17, 100, 383, 699, 700] {
            let expected = data.get(offset..).ok_or("offset in range")?;
            assert_eq!(
                read_from(&archive, index, offset as u64)?,
                expected,
                "Entry {index} from {offset}"
            );
        }
    }
    Ok(())
}

#[test]
fn an_offset_beyond_the_end_is_an_error() -> TestResult {
    let (_dir, archive) = archive(vec![TestEntry::stored("Data\\a.bin", data())])?;
    let entry = archive.entries().first().ok_or("Entry missing")?;
    let result = archive.open_entry_at(entry, 701);
    assert!(
        matches!(
            result,
            Err(ArchiveError::OffsetBeyondEnd {
                offset: 701,
                size: 700,
                ..
            })
        ),
        "beyond the end: {result:?}"
    );
    Ok(())
}

#[test]
fn a_compressed_entry_decodes_at_most_the_skip_limit_to_reach_an_offset() -> TestResult {
    let (_dir, archive) = archive(vec![
        TestEntry::deflated("Data\\huge.bin", data()).with_uncompressed_size(2 * MAX_DECODED_SKIP),
    ])?;
    let compressed = archive.entries().first().ok_or("Entry missing")?;
    let result = archive.open_entry_at(compressed, MAX_DECODED_SKIP + 1);
    assert!(
        matches!(
            &result,
            Err(ArchiveError::SkipTooLong { limit, .. }) if *limit == MAX_DECODED_SKIP
        ),
        "the skip is refused before decoding: {result:?}"
    );
    let message = result.err().map_or_default(|error| error.to_string());
    assert!(message.contains("compressed"), "{message}");
    Ok(())
}
