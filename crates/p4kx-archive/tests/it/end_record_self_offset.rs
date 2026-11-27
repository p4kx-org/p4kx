// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_archive::{Archive, ArchiveMetadataStatus, EntryIssue, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

const END_LEN: usize = 22;
const DIRECTORY_OFFSET_AT: usize = 16;
const COMMENT_LEN_AT: usize = 20;

fn missing_end_with_self_aligned_payload() -> Result<[Vec<u8>; 2], Box<dyn Error>> {
    let name = "empty.socpak";
    let mut empty = TestContainer::new().build();
    let provisional = TestContainer::new()
        .entry(TestEntry::stored(name, empty.as_slice()))
        .build();
    let payload = provisional
        .windows(4)
        .position(|bytes| bytes == b"PK\x05\x06")
        .ok_or("original stored payload END")?;
    empty
        .get_mut(DIRECTORY_OFFSET_AT..COMMENT_LEN_AT)
        .ok_or("inner directory offset")?
        .copy_from_slice(&u32::try_from(payload)?.to_le_bytes());
    let plain = TestContainer::new()
        .entry(TestEntry::stored(name, empty.as_slice()))
        .build();
    let zip64 = ArchiveBuilder::new()
        .entry(TestEntry::stored(name, empty.as_slice()))
        .build();
    let mut variants = [plain, zip64];
    for bytes in &mut variants {
        assert_eq!(
            bytes.windows(4).position(|part| part == b"PK\x05\x06"),
            Some(payload),
            "the stored inner directory offset equals its physical END position"
        );
        let outer = bytes
            .windows(4)
            .rposition(|part| part == b"PK\x05\x06")
            .ok_or("original outer END")?;
        assert!(outer > payload, "there is a distinct outer end frame");
        bytes.truncate(outer);
    }
    Ok(variants)
}

#[test]
fn a_self_aligned_empty_payload_end_does_not_certify_the_missing_outer_directory() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    for bytes in missing_end_with_self_aligned_payload()? {
        std::fs::write(&path, bytes)?;
        assert!(
            Archive::open(&path).is_err(),
            "self-aligned payload words do not prove an original empty root"
        );
    }
    Ok(())
}

#[test]
fn a_self_aligned_empty_payload_end_still_requires_explicit_outer_recovery() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    for bytes in missing_end_with_self_aligned_payload()? {
        std::fs::write(&path, bytes)?;
        let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
        assert_eq!(
            archive.entries().len(),
            1,
            "the outer source member survives"
        );
        assert!(
            archive
                .issues()
                .iter()
                .any(|(_, issue)| *issue == EntryIssue::Recovered),
            "recovered membership retains explicit uncertainty"
        );
        assert_eq!(
            archive.metadata().status(),
            ArchiveMetadataStatus::Recovered,
            "a payload END cannot suppress required recovery"
        );
    }
    Ok(())
}

#[test]
fn an_empty_zip64_root_retains_a_nonempty_zip_as_its_binary_comment() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let comment = TestContainer::new()
        .entry(TestEntry::stored("inner", b"data"))
        .build();
    let mut bytes = ArchiveBuilder::new().build();
    let end = bytes
        .windows(4)
        .rposition(|part| part == b"PK\x05\x06")
        .ok_or("original root END")?;
    bytes.truncate(end + END_LEN);
    bytes
        .get_mut(end + COMMENT_LEN_AT..end + END_LEN)
        .ok_or("original comment length")?
        .copy_from_slice(&u16::try_from(comment.len())?.to_le_bytes());
    bytes.extend_from_slice(&comment);
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert!(
        archive.entries().is_empty(),
        "the comment does not become root membership"
    );
    assert!(
        archive.issues().is_empty(),
        "the original empty root needs no recovery"
    );
    assert_eq!(
        archive.metadata().status(),
        ArchiveMetadataStatus::Available
    );
    let zip = archive
        .metadata()
        .zip_directory()
        .ok_or("original ZIP64 route")?;
    assert_eq!(
        zip.end().fixed().offset(),
        u64::try_from(end)?,
        "original classic END"
    );
    assert_eq!(zip.zip64().ok_or("ZIP64 end frame")?.fixed().offset(), 0);
    assert_eq!(zip.end().comment_len(), u16::try_from(comment.len())?);
    Ok(())
}

#[derive(Clone, Copy)]
enum StoredRange {
    EnclosesEnd,
    OutsideSource,
}

fn empty_generation_with_trailing_record(
    name: [u8; 5],
    range: StoredRange,
) -> Result<(Vec<u8>, usize), Box<dyn Error>> {
    let mut bytes = TestContainer::new()
        .entry(TestEntry::stored("owner", b"value"))
        .build();
    let central = bytes
        .windows(4)
        .position(|part| part == b"PK\x01\x02")
        .ok_or("original directory record")?;
    let old_end = bytes
        .len()
        .checked_sub(END_LEN)
        .ok_or("original end frame")?;
    let mut record = bytes
        .get(central..old_end)
        .ok_or("original record bytes")?
        .to_vec();
    record
        .get_mut(46..51)
        .ok_or("original five-byte name")?
        .copy_from_slice(&name);
    let end = bytes.len();
    let mut empty = TestContainer::new().build();
    empty
        .get_mut(DIRECTORY_OFFSET_AT..COMMENT_LEN_AT)
        .ok_or("new directory offset")?
        .copy_from_slice(&u32::try_from(end)?.to_le_bytes());
    bytes.extend_from_slice(&empty);
    let data_start = 30 + b"owner".len();
    let stored = match range {
        StoredRange::EnclosesEnd => u32::try_from(bytes.len() - data_start)?,
        StoredRange::OutsideSource => u32::MAX,
    };
    record
        .get_mut(20..24)
        .ok_or("trailing declared stored size")?
        .copy_from_slice(&stored.to_le_bytes());
    bytes.extend_from_slice(&record);
    Ok((bytes, end))
}

#[test]
fn an_unbound_central_looking_trailer_does_not_own_an_empty_generation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (bytes, end) = empty_generation_with_trailing_record(*b"other", StoredRange::EnclosesEnd)?;
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert!(
        archive.entries().is_empty(),
        "different local names cannot invent a containing Entry"
    );
    let zip = archive
        .metadata()
        .zip_directory()
        .ok_or("selected empty route")?;
    assert_eq!(
        zip.end().fixed().offset(),
        u64::try_from(end)?,
        "the latest real generation is retained"
    );
    Ok(())
}

#[test]
fn an_unbounded_stored_declaration_cannot_be_physical_frame_authority() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (bytes, end) =
        empty_generation_with_trailing_record(*b"owner", StoredRange::OutsideSource)?;
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert!(
        archive.entries().is_empty(),
        "an out-of-source range does not certify a containing Entry"
    );
    let zip = archive
        .metadata()
        .zip_directory()
        .ok_or("selected empty route")?;
    assert_eq!(
        zip.end().fixed().offset(),
        u64::try_from(end)?,
        "the actual empty end frame is retained"
    );
    Ok(())
}

#[test]
fn a_bound_trailing_record_overlapping_an_empty_end_is_explicitly_ambiguous() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (bytes, _) = empty_generation_with_trailing_record(*b"owner", StoredRange::EnclosesEnd)?;
    std::fs::write(&path, bytes)?;
    assert!(
        matches!(
            Archive::open(&path),
            Err(p4kx_archive::ArchiveError::Corrupt { .. })
        ),
        "a fully bound competing physical frame cannot silently certify empty membership"
    );
    Ok(())
}
