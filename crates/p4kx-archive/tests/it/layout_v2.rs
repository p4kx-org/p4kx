// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Opening and reading Archives in the v2 layout through the public API.

use std::error::Error;
use std::io::Read;
use std::path::Path;

use p4kx_archive::{Archive, ArchiveError, Compression};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};

type TestResult = Result<(), Box<dyn Error>>;

const TRAILER_LEN: usize = 175;

fn entries() -> Vec<TestEntry> {
    vec![
        TestEntry::stored(
            "Data\\Libs\\Config\\defaultProfile.xml",
            b"<Profile/>".to_vec(),
        ),
        TestEntry::deflated("Data\\a.xml", b"<deflate/>".to_vec()),
        TestEntry::zstd("Data\\b.xml", b"<zstd/>".to_vec()).encrypted(),
        TestEntry::zstd_without_magic("Data\\c.xml", b"<plain/>".to_vec()),
        TestEntry::stored("Data\\Scripts\\empty.lua", Vec::new()),
    ]
}

fn v2_archive() -> Vec<u8> {
    entries()
        .into_iter()
        .fold(ArchiveBuilder::v2(), ArchiveBuilder::entry)
        .build()
}

fn open_bytes(dir: &Path, bytes: &[u8]) -> Result<Result<Archive, ArchiveError>, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    std::fs::write(&path, bytes)?;
    Ok(Archive::open_with_key(&path, Some(test_key())))
}

fn summary(archive: &Archive) -> Vec<(String, u64, u64, Compression, bool)> {
    archive
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.path().to_owned(),
                entry.compressed_size(),
                entry.uncompressed_size(),
                entry.compression(),
                entry.is_encrypted(),
            )
        })
        .collect()
}

fn read_all(archive: &Archive, index: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let entry = archive.entries().get(index).ok_or("Entry missing")?;
    let mut data = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut data)?;
    Ok(data)
}

fn put_u64(bytes: &mut [u8], at: usize, value: u64) -> TestResult {
    bytes
        .get_mut(at..at + 8)
        .ok_or("field outside the synthetic Archive")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn trailer_field(bytes: &[u8], field: usize) -> Result<u64, Box<dyn Error>> {
    let at = bytes.len() - TRAILER_LEN + field;
    let raw = bytes.get(at..at + 8).ok_or("trailer field missing")?;
    Ok(u64::from_le_bytes(raw.try_into()?))
}

#[test]
fn a_v2_archive_lists_the_same_entries_as_a_v1_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let v1 = entries()
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .build();

    let from_v1 = summary(&open_bytes(dir.path(), &v1)??);
    let from_v2 = summary(&open_bytes(dir.path(), &v2_archive())??);

    assert_eq!(from_v2, from_v1, "v2 and v1 Archives list identically");
    assert_eq!(
        from_v2
            .iter()
            .map(|entry| entry.0.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Data\\Libs\\Config\\defaultProfile.xml",
            "Data\\a.xml",
            "Data\\b.xml",
            "Data\\c.xml",
            "Data\\Scripts\\empty.lua",
        ],
        "Entries in directory order"
    );
    Ok(())
}

#[test]
fn a_v2_archive_reads_compressed_and_encrypted_entries() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open_bytes(dir.path(), &v2_archive())??;
    let expected: [&[u8]; 5] = [b"<Profile/>", b"<deflate/>", b"<zstd/>", b"<plain/>", b""];

    for (index, expected) in expected.iter().enumerate() {
        assert_eq!(
            read_all(&archive, index)?,
            *expected,
            "data of Entry {index}"
        );
    }
    Ok(())
}

#[test]
fn a_v2_archive_laid_out_by_hand_is_read_at_the_documented_offsets() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = b"armor".to_vec();
    bytes.extend_from_slice(&5_u64.to_le_bytes());
    let mut record = vec![0_u8; 204];
    put_u64(&mut record, 0x0A, 5)?;
    put_u64(&mut record, 0x12, 5)?;
    bytes.extend(record);
    let names = b"Data\\Objects\\test.skin\0";
    bytes.extend_from_slice(names);
    let mut trailer = vec![0_u8; TRAILER_LEN];
    put_u64(&mut trailer, 0x00, 1)?;
    put_u64(&mut trailer, 0x10, 13)?;
    put_u64(&mut trailer, 0x18, 204)?;
    put_u64(&mut trailer, 0x28, 217)?;
    put_u64(&mut trailer, 0x30, names.len() as u64)?;
    put_u64(&mut trailer, 0x40, 5)?;
    trailer
        .get_mut(0xA9..)
        .ok_or("trailer too short")?
        .copy_from_slice(&[2, 0, b'J', b'i', b'j', b'i']);
    bytes.extend(trailer);

    let archive = open_bytes(dir.path(), &bytes)??;

    assert_eq!(
        summary(&archive),
        vec![(
            "Data\\Objects\\test.skin".to_owned(),
            5,
            5,
            Compression::Store,
            false
        )],
        "the single Entry"
    );
    assert_eq!(read_all(&archive, 0)?, b"armor", "Entry data");
    Ok(())
}

#[test]
fn v2_entry_paths_use_backslash_separators() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bytes = ArchiveBuilder::v2()
        .entry(TestEntry::stored(
            "Data/Objects/test.skin",
            b"armor".to_vec(),
        ))
        .build();

    let archive = open_bytes(dir.path(), &bytes)??;

    assert_eq!(
        archive.entries().first().map(p4kx_archive::Entry::path),
        Some("Data\\Objects\\test.skin"),
        "forward slashes become backslashes"
    );
    assert_eq!(read_all(&archive, 0)?, b"armor", "Entry data");
    Ok(())
}

#[test]
fn a_v2_archive_lists_more_entries_than_a_16_bit_count_can_hold() -> TestResult {
    let dir = tempfile::tempdir()?;
    let count = 70_000;
    let bytes = (0..count)
        .fold(ArchiveBuilder::v2(), |builder, index| {
            builder.entry(TestEntry::stored(
                format!("Data\\Many\\{index:05}.txt"),
                Vec::new(),
            ))
        })
        .build();

    let archive = open_bytes(dir.path(), &bytes)??;

    assert_eq!(archive.entries().len(), count, "every Entry is listed");
    assert_eq!(
        archive.entries().last().map(p4kx_archive::Entry::path),
        Some("Data\\Many\\69999.txt"),
        "the last name is read from the name table"
    );
    Ok(())
}

#[test]
fn an_unknown_layout_version_is_reported_by_its_number() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = v2_archive();
    let version = bytes.len() - 6;
    bytes
        .get_mut(version..version + 2)
        .ok_or("no layout version")?
        .copy_from_slice(&3_u16.to_le_bytes());

    let result = open_bytes(dir.path(), &bytes)?;

    assert!(
        matches!(
            result,
            Err(ArchiveError::UnsupportedLayout { version: 3, .. })
        ),
        "layout version 3 is not supported: {result:?}"
    );
    let message = result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        message.contains("layout v3"),
        "the message names the layout: {message}"
    );
    Ok(())
}

#[test]
fn a_v2_directory_size_that_does_not_match_the_entry_count_is_corrupt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = v2_archive();
    let size = trailer_field(&bytes, 24)?;
    let at = bytes.len() - TRAILER_LEN + 24;
    put_u64(&mut bytes, at, size - 1)?;

    let result = open_bytes(dir.path(), &bytes)?;

    assert!(
        matches!(result, Err(ArchiveError::Corrupt { ref reason, .. }) if reason.contains("directory")),
        "the directory size must be 204 bytes per Entry: {result:?}"
    );
    Ok(())
}

#[test]
fn a_v2_directory_outside_the_file_is_corrupt() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = v2_archive();
    let at = bytes.len() - TRAILER_LEN + 16;
    put_u64(&mut bytes, at, u64::MAX - 8)?;

    let result = open_bytes(dir.path(), &bytes)?;

    assert!(
        matches!(result, Err(ArchiveError::Corrupt { .. })),
        "a directory beyond the end of the file is corrupt: {result:?}"
    );
    Ok(())
}

#[test]
fn a_v2_name_offset_outside_the_name_table_is_reported_at_its_record() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = v2_archive();
    let directory = usize::try_from(trailer_field(&bytes, 16)?)?;
    let second_record = directory + 204;
    put_u64(&mut bytes, second_record + 0x22, 1 << 40)?;

    let result = open_bytes(dir.path(), &bytes)?;

    assert!(
        matches!(result, Err(ArchiveError::Corrupt { offset, .. }) if offset == second_record as u64),
        "the record at {second_record} names a missing path: {result:?}"
    );
    Ok(())
}

#[test]
fn an_empty_v2_entry_path_is_reported_at_its_record() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bytes = ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data\\a.xml", b"<a/>".to_vec()))
        .entry(TestEntry::stored("", b"<b/>".to_vec()))
        .build();
    let directory = usize::try_from(trailer_field(&bytes, 16)?)?;
    let second_record = directory + 204;

    let result = open_bytes(dir.path(), &bytes)?;

    assert!(
        matches!(result, Err(ArchiveError::Corrupt { offset, ref reason, .. }) if offset == second_record as u64 && reason.contains("empty")),
        "the record at {second_record} has an empty path: {result:?}"
    );
    Ok(())
}

#[test]
fn v2_entry_data_outside_the_file_is_corrupt_when_opened() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = v2_archive();
    let directory = usize::try_from(trailer_field(&bytes, 16)?)?;
    put_u64(&mut bytes, directory + 0x1A, 1 << 40)?;
    let archive = open_bytes(dir.path(), &bytes)??;
    let entry = archive.entries().first().ok_or("Entry missing")?;

    let result = archive.open_entry(entry);

    assert!(
        matches!(result, Err(ArchiveError::Corrupt { offset, .. }) if offset == 1 << 40),
        "data beyond the end of the file is corrupt: {result:?}"
    );
    Ok(())
}
