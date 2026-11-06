// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Independent base/extension directory fixtures through the public Archive API.

use std::error::Error;
use std::io::Read;

use p4kx_archive::{Archive, ArchiveError, MAX_CONTAINER_ENTRIES};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

const TRAILER_LEN: usize = 175;
const RECORD_LEN: usize = 204;
const PAYLOAD_LEN: usize = 6;
const NAMES: &[u8] = b"Data/a.bin\0Data/b.bin\0";

fn put(bytes: &mut [u8], at: usize, value: u64) -> TestResult {
    bytes
        .get_mut(at..at + 8)
        .ok_or("synthetic u64 field")?
        .copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn field(bytes: &mut [u8], offset: usize, value: u64) -> TestResult {
    let at = bytes.len() - TRAILER_LEN + offset;
    put(bytes, at, value)
}

fn fixture(base: u64, extension: u64) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = b"abcabc".to_vec();
    for index in 0..2 {
        let mut record = vec![0; RECORD_LEN];
        record
            .get_mut(6..10)
            .ok_or("synthetic checksum field")?
            .copy_from_slice(&crc32c::crc32c(b"abc").to_le_bytes());
        put(&mut record, 10, 3)?;
        put(&mut record, 18, 3)?;
        put(&mut record, 26, index * 3)?;
        put(&mut record, 34, index * 11)?;
        bytes.extend(record);
    }
    bytes.extend_from_slice(NAMES);
    bytes.extend_from_slice(&[0; TRAILER_LEN]);
    field(&mut bytes, 0, base)?;
    field(&mut bytes, 8, extension)?;
    field(&mut bytes, 16, PAYLOAD_LEN as u64)?;
    field(&mut bytes, 24, base * RECORD_LEN as u64)?;
    field(&mut bytes, 32, extension * RECORD_LEN as u64)?;
    field(&mut bytes, 40, (PAYLOAD_LEN + 2 * RECORD_LEN) as u64)?;
    field(&mut bytes, 48, base * 11)?;
    field(&mut bytes, 56, extension * 11)?;
    let marker = bytes.len() - 6;
    bytes
        .get_mut(marker..)
        .ok_or("synthetic layout marker")?
        .copy_from_slice(&[2, 0, b'J', b'i', b'j', b'i']);
    Ok(bytes)
}

fn check_payloads(bytes: &[u8]) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    let paths: Vec<_> = archive
        .entries()
        .iter()
        .map(p4kx_archive::Entry::path)
        .collect();
    assert_eq!(
        paths,
        ["Data\\a.bin", "Data\\b.bin"],
        "base and extension paths"
    );
    for entry in archive.entries() {
        let mut output = Vec::new();
        archive
            .open_entry_verified(entry)?
            .read_to_end(&mut output)?;
        assert_eq!(output, b"abc", "each payload verifies independently");
    }
    Ok(())
}

fn corrupt(bytes: &[u8]) -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    std::fs::write(&path, bytes)?;
    assert!(
        matches!(Archive::open(&path), Err(ArchiveError::Corrupt { .. })),
        "invalid declarations must not expose a complete base subset"
    );
    Ok(())
}

#[test]
fn base_extension_and_extension_only_have_identical_verified_payloads() -> TestResult {
    for (base, extension) in [(2, 0), (1, 1), (0, 2)] {
        check_payloads(&fixture(base, extension)?)?;
    }
    Ok(())
}

#[test]
fn each_directory_section_must_match_its_own_count() -> TestResult {
    let mut bytes = fixture(1, 1)?;
    field(&mut bytes, 32, 0)?;
    corrupt(&bytes)?;
    field(&mut bytes, 24, 2 * RECORD_LEN as u64)?;
    corrupt(&bytes)?;
    Ok(())
}

#[test]
fn overflowing_counts_sizes_and_ranges_are_rejected() -> TestResult {
    for (offset, value) in [
        (0, u64::MAX),
        (8, u64::MAX),
        (24, u64::MAX),
        (32, u64::MAX),
        (48, u64::MAX),
        (56, u64::MAX),
        (16, u64::MAX),
        (40, u64::MAX),
    ] {
        let mut bytes = fixture(1, 1)?;
        field(&mut bytes, offset, value)?;
        corrupt(&bytes)?;
    }
    Ok(())
}

#[test]
fn overlapping_or_nonadjacent_directory_and_names_are_rejected() -> TestResult {
    for names_offset in [
        PAYLOAD_LEN as u64,
        (PAYLOAD_LEN + 2 * RECORD_LEN - 1) as u64,
    ] {
        let mut bytes = fixture(1, 1)?;
        field(&mut bytes, 40, names_offset)?;
        corrupt(&bytes)?;
    }
    Ok(())
}

#[test]
fn names_and_extension_data_must_be_within_their_declared_ranges() -> TestResult {
    let mut outside = fixture(1, 1)?;
    put(
        &mut outside,
        PAYLOAD_LEN + RECORD_LEN + 34,
        NAMES.len() as u64,
    )?;
    corrupt(&outside)?;

    let mut missing_names = fixture(1, 1)?;
    field(&mut missing_names, 56, 0)?;
    corrupt(&missing_names)?;

    let mut unterminated = fixture(1, 1)?;
    let final_name = unterminated.len() - TRAILER_LEN - 1;
    *unterminated.get_mut(final_name).ok_or("final name byte")? = b'x';
    corrupt(&unterminated)?;

    let mut truncated = fixture(1, 1)?;
    truncated.remove(PAYLOAD_LEN + RECORD_LEN);
    corrupt(&truncated)?;
    Ok(())
}

#[test]
fn an_empty_v2_archive_has_valid_empty_sections() -> TestResult {
    let mut bytes = vec![0; TRAILER_LEN];
    let marker = bytes.len() - 6;
    bytes
        .get_mut(marker..)
        .ok_or("synthetic layout marker")?
        .copy_from_slice(&[2, 0, b'J', b'i', b'j', b'i']);
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    std::fs::write(&path, &bytes)?;
    let archive = Archive::open(&path)?;
    assert_eq!(archive.entries().len(), 0, "empty sections have no Entries");
    Ok(())
}

#[test]
fn the_nested_entry_limit_applies_to_base_plus_extension_before_allocation() -> TestResult {
    let mut bytes = fixture(1, 1)?;
    let extensions = u64::try_from(MAX_CONTAINER_ENTRIES)?;
    field(&mut bytes, 8, extensions)?;
    field(&mut bytes, 32, extensions * RECORD_LEN as u64)?;
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\test.socpak", bytes))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let entry = archive.entries().first().ok_or("container Entry")?;
    let failure = archive
        .open_container(entry)
        .err()
        .ok_or("limit was ignored")?;
    assert!(
        matches!(failure, ArchiveError::Container { source, .. }
            if matches!(*source, ArchiveError::LimitExceeded { .. })),
        "the combined Entry limit is checked before allocating"
    );
    Ok(())
}
