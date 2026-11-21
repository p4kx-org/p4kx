// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The CIG hash block of a v1 directory record: a block of another length is reported, and the
//! last of several decides.

use std::error::Error;

use p4kx_archive::{Archive, EntryIssue, MetadataRange, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

fn open(entry: TestEntry) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    let archive = Archive::open(&path)?;
    Ok((dir, archive))
}

fn entry() -> TestEntry {
    TestEntry::stored("Data\\a.bin", b"<a/>".to_vec())
}

fn stated(archive: &Archive) -> Result<Option<[u8; 32]>, Box<dyn Error>> {
    Ok(archive.entries().first().ok_or("no Entry")?.sha256())
}

#[test]
fn a_hash_block_of_another_length_is_reported_and_states_no_digest() -> TestResult {
    for (data, size) in [(vec![7_u8; 31], 35), (vec![7_u8; 33], 37), (Vec::new(), 4)] {
        let (_dir, archive) = open(entry().with_hash_block(data))?;

        assert_eq!(archive.issues(), [(0, EntryIssue::HashBlockSize { size })]);
        assert_eq!(stated(&archive)?, None, "no digest");
    }
    Ok(())
}

#[test]
fn a_hash_block_of_32_bytes_is_no_issue_even_when_all_zero() -> TestResult {
    for data in [vec![7_u8; 32], vec![0_u8; 32]] {
        let (_dir, archive) = open(entry().with_hash_block(data))?;

        assert_eq!(archive.issue_count(), 0);
    }
    Ok(())
}

#[test]
fn the_last_of_several_hash_blocks_decides() -> TestResult {
    let (_dir, archive) = open(
        entry()
            .with_hash_block([1; 32])
            .with_extra_hash_block(&[2; 32]),
    )?;
    assert_eq!(stated(&archive)?, Some([2; 32]), "the later digest wins");
    assert_eq!(archive.issue_count(), 0);

    let (_dir, archive) = open(
        entry()
            .with_hash_block([1; 32])
            .with_extra_hash_block(&[2; 31]),
    )?;
    assert_eq!(stated(&archive)?, None, "a later malformed block wins too");
    assert_eq!(
        archive.issues(),
        [(0, EntryIssue::HashBlockSize { size: 35 })]
    );

    let (_dir, archive) = open(
        entry()
            .with_hash_block([1; 31])
            .with_extra_hash_block(&[2; 32]),
    )?;
    assert_eq!(stated(&archive)?, Some([2; 32]), "a later valid block wins");
    assert_eq!(
        archive.issues(),
        [(0, EntryIssue::HashBlockSize { size: 35 })],
        "the earlier malformed block is still reported"
    );
    Ok(())
}
#[test]
fn the_first_defective_hash_header_retains_its_original_words() -> TestResult {
    let (_dir, archive) = open(
        entry()
            .with_hash_block([1; 2])
            .with_extra_hash_block(&[2; 3])
            .with_extra_hash_block(&[3; 32]),
    )?;
    let data = std::fs::read(archive.path())?;
    check_hash_header(&archive, &data, 0)?;
    assert_eq!(
        stated(&archive)?,
        Some([3; 32]),
        "the final valid digest still wins"
    );
    assert!(
        archive.observed_hash_block_size(1).is_none(),
        "only the first size defect is listed"
    );
    Ok(())
}

#[test]
fn recovery_preserves_the_detected_hash_header_at_its_original_offset() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("recovered.p4k");
    let mut data = ArchiveBuilder::new()
        .entry(entry().with_hash_block([1; 2]))
        .build();
    let end_signature = 0x0606_4b50_u32.to_le_bytes();
    let end = data
        .windows(end_signature.len())
        .rposition(|window| window == end_signature)
        .ok_or("ZIP64 end record")?;
    data.truncate(end);
    std::fs::write(&path, &data)?;
    let archive = Archive::open_with(path, OpenOptions::new().recover(true))?;
    assert_eq!(
        archive.issues()[0],
        (0, EntryIssue::Recovered),
        "the scan added its own first issue"
    );
    assert!(
        archive.observed_hash_block_size(0).is_none(),
        "the archive-wide issue has no hash header"
    );
    check_hash_header(&archive, &data, 1)
}

fn check_hash_header(archive: &Archive, data: &[u8], position: usize) -> TestResult {
    assert_eq!(
        archive.issues().get(position),
        Some(&(0, EntryIssue::HashBlockSize { size: 6 })),
        "the retained issue identifies the first malformed hash block"
    );
    let header = archive
        .observed_hash_block_size(position)
        .ok_or("original hash header")?;
    assert_eq!(word(data, header.id())?, 0x5003, "the parsed type word");
    assert_eq!(
        word(data, header.declared_len())?,
        6,
        "the first defective length word"
    );
    assert_eq!(
        header.id().offset() + header.id().len(),
        header.declared_len().offset(),
        "words are adjacent"
    );
    Ok(())
}

fn word(data: &[u8], range: MetadataRange) -> Result<u16, Box<dyn Error>> {
    let start = usize::try_from(range.offset())?;
    let end = start + usize::try_from(range.len())?;
    Ok(u16::from_le_bytes(
        data.get(start..end).ok_or("word range")?.try_into()?,
    ))
}
