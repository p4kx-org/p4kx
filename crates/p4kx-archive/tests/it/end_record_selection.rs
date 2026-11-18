// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_archive::{Archive, ArchiveMetadataStatus, EntryIssue, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

const END_RECORD_LEN: usize = 22;

fn missing_outer_end_records() -> Result<[Vec<u8>; 2], Box<dyn Error>> {
    let empty = TestContainer::new().build();
    let plain = TestContainer::new()
        .entry(TestEntry::stored("empty.socpak", empty.as_slice()))
        .build();
    let zip64 = ArchiveBuilder::new()
        .entry(TestEntry::stored("empty.socpak", empty.as_slice()))
        .build();
    let mut variants = [plain, zip64];
    for bytes in &mut variants {
        let outer_end = bytes
            .windows(4)
            .rposition(|part| part == b"PK\x05\x06")
            .ok_or("outer end record missing")?;
        bytes.truncate(outer_end);
    }
    Ok(variants)
}

#[test]
fn a_missing_outer_end_cannot_certify_an_inner_empty_zip_inventory() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    for bytes in missing_outer_end_records()? {
        std::fs::write(&path, bytes)?;
        assert!(
            Archive::open(&path).is_err(),
            "an end record inside a stored payload does not locate the outer directory"
        );
    }
    Ok(())
}

#[test]
fn a_missing_outer_end_recovers_real_records_with_explicit_uncertainty() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    for bytes in missing_outer_end_records()? {
        std::fs::write(&path, bytes)?;
        let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
        assert_eq!(archive.entries().len(), 1, "the outer member survives");
        assert!(
            archive
                .issues()
                .iter()
                .any(|(_, issue)| *issue == EntryIssue::Recovered),
            "recovered membership cannot certify complete absence"
        );
        assert_eq!(
            archive.metadata().status(),
            ArchiveMetadataStatus::Recovered,
            "the selected original layout is unknown after recovery"
        );
    }
    Ok(())
}

#[test]
fn an_empty_end_record_in_a_declared_comment_does_not_replace_its_outer_directory() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let mut bytes = TestContainer::new()
        .entry(TestEntry::stored("member", b"value"))
        .build();
    let empty = TestContainer::new().build();
    let end = bytes.len().checked_sub(22).ok_or("outer end fixed size")?;
    bytes
        .get_mut(end + 20..end + 22)
        .ok_or("outer comment length")?
        .copy_from_slice(&u16::try_from(empty.len())?.to_le_bytes());
    bytes.extend_from_slice(&empty);
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert_eq!(
        archive.entries().len(),
        1,
        "the outer directory still owns its member"
    );
    let zip = archive
        .metadata()
        .zip_directory()
        .ok_or("original ZIP route")?;
    assert_eq!(zip.end().fixed().offset(), u64::try_from(end)?);
    assert_eq!(zip.end().comment_len(), 22);
    assert!(
        archive.issues().is_empty(),
        "a binary comment is not recovery"
    );
    Ok(())
}

#[test]
fn genuine_empty_archives_and_opaque_following_bytes_remain_readable() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    for mut bytes in [TestContainer::new().build(), ArchiveBuilder::new().build()] {
        for following in [b"".as_slice(), b"PAD".as_slice()] {
            bytes.extend_from_slice(following);
            std::fs::write(&path, &bytes)?;
            let archive = Archive::open(&path)?;
            assert!(
                archive.entries().is_empty(),
                "the empty original directory is retained"
            );
            assert!(
                archive.issues().is_empty(),
                "opaque following bytes are not recovery"
            );
            assert_eq!(
                archive.metadata().status(),
                ArchiveMetadataStatus::Available
            );
        }
    }
    Ok(())
}

fn competing_comment(comment_len: u16) -> Result<(Vec<u8>, usize), Box<dyn Error>> {
    let mut bytes = TestContainer::new()
        .entry(TestEntry::stored("member", b"value"))
        .build();
    let end = bytes.len().checked_sub(END_RECORD_LEN).ok_or("outer end")?;
    let fake = bytes.get(end..).ok_or("original end bytes")?.to_vec();
    bytes
        .get_mut(end + 20..end + END_RECORD_LEN)
        .ok_or("original comment length")?
        .copy_from_slice(&comment_len.to_le_bytes());
    bytes.extend_from_slice(&fake);
    Ok((bytes, end))
}

#[test]
fn a_comment_record_reusing_real_directory_words_remains_comment_data() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (bytes, end) = competing_comment(u16::try_from(END_RECORD_LEN)?)?;
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    let zip = archive.metadata().zip_directory().ok_or("ZIP route")?;
    assert_eq!(zip.end().fixed().offset(), u64::try_from(end)?);
    assert_eq!(zip.end().comment_len(), u16::try_from(END_RECORD_LEN)?);
    assert_eq!(
        archive.entries().len(),
        1,
        "the authored member is retained"
    );
    Ok(())
}

#[test]
fn a_truncated_comment_keeps_its_original_route_and_available_prefix() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (bytes, end) = competing_comment(u16::MAX)?;
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    let zip = archive.metadata().zip_directory().ok_or("ZIP route")?;
    assert_eq!(zip.end().fixed().offset(), u64::try_from(end)?);
    assert_eq!(zip.end().comment_len(), u16::MAX);
    assert_eq!(
        archive.entries().len(),
        1,
        "malformed comment is separate from membership"
    );
    assert_eq!(
        archive.metadata().status(),
        ArchiveMetadataStatus::Malformed
    );
    Ok(())
}

#[test]
fn a_later_empty_generation_outside_an_old_comment_remains_authoritative() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (mut bytes, _) = competing_comment(u16::try_from(END_RECORD_LEN)?)?;
    let end = bytes.len();
    let mut latest = TestContainer::new().build();
    latest
        .get_mut(16..20)
        .ok_or("empty generation directory offset")?
        .copy_from_slice(&u32::try_from(end)?.to_le_bytes());
    bytes.extend_from_slice(&latest);
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert!(
        archive.entries().is_empty(),
        "the later generation intentionally has no members"
    );
    let zip = archive.metadata().zip_directory().ok_or("ZIP route")?;
    assert_eq!(zip.end().fixed().offset(), u64::try_from(end)?);
    assert!(
        archive.issues().is_empty(),
        "ordinary generation selection is not recovery"
    );
    Ok(())
}

#[test]
fn empty_prefixed_zip_offsets_are_relative_to_the_owning_source() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let prefix = b"MZ-synthetic-prefix";
    let mut bytes = prefix.to_vec();
    bytes.extend_from_slice(&TestContainer::new().build());
    std::fs::write(&path, &bytes)?;
    assert!(
        matches!(
            Archive::open(&path),
            Err(p4kx_archive::ArchiveError::Corrupt { .. })
        ),
        "an unadjusted relative ZIP offset is not silently rebased after a prefix"
    );
    bytes
        .get_mut(prefix.len() + 16..prefix.len() + 20)
        .ok_or("source-relative directory offset")?
        .copy_from_slice(&u32::try_from(prefix.len())?.to_le_bytes());
    std::fs::write(&path, &bytes)?;
    let archive = Archive::open(&path)?;
    assert!(
        archive.entries().is_empty(),
        "the source-relative empty directory is readable"
    );
    assert_eq!(
        archive.metadata().status(),
        ArchiveMetadataStatus::Available
    );
    Ok(())
}

#[test]
fn a_payload_end_claiming_the_outer_tail_as_comment_is_explicitly_ambiguous() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let name = "member";
    let mut fake = TestContainer::new().build();
    let provisional = TestContainer::new()
        .entry(TestEntry::stored(name, fake.as_slice()))
        .build();
    let payload = provisional
        .windows(4)
        .position(|part| part == b"PK\x05\x06")
        .ok_or("authored payload end record")?;
    fake.get_mut(16..20)
        .ok_or("fake directory offset")?
        .copy_from_slice(&u32::try_from(payload)?.to_le_bytes());
    let comment_len = provisional.len() - payload - END_RECORD_LEN;
    fake.get_mut(20..END_RECORD_LEN)
        .ok_or("fake comment length")?
        .copy_from_slice(&u16::try_from(comment_len)?.to_le_bytes());
    let bytes = TestContainer::new()
        .entry(TestEntry::stored(name, fake.as_slice()))
        .build();
    std::fs::write(&path, bytes)?;
    assert!(
        matches!(
            Archive::open(&path),
            Err(p4kx_archive::ArchiveError::Corrupt { .. })
        ),
        "overlapping Entry and comment ownership must not certify a healthy empty source"
    );
    Ok(())
}

#[test]
fn an_empty_root_can_hold_a_nonempty_zip_as_its_binary_comment() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let comment = TestContainer::new()
        .entry(TestEntry::stored("inner", b"data"))
        .build();
    let mut bytes = TestContainer::new().build();
    bytes
        .get_mut(20..END_RECORD_LEN)
        .ok_or("root comment length")?
        .copy_from_slice(&u16::try_from(comment.len())?.to_le_bytes());
    bytes.extend_from_slice(&comment);
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert!(
        archive.entries().is_empty(),
        "the nested comment does not become the root"
    );
    let zip = archive
        .metadata()
        .zip_directory()
        .ok_or("original root route")?;
    assert_eq!(zip.end().fixed().offset(), 0);
    assert_eq!(zip.end().comment_len(), u16::try_from(comment.len())?);
    Ok(())
}

#[test]
fn an_invalid_later_end_outside_an_old_comment_is_not_a_silent_rollback() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (mut bytes, _) = competing_comment(u16::try_from(END_RECORD_LEN)?)?;
    bytes.extend_from_slice(&TestContainer::new().build());
    std::fs::write(&path, bytes)?;
    assert!(
        matches!(
            Archive::open(&path),
            Err(p4kx_archive::ArchiveError::Corrupt { .. })
        ),
        "an invalid later generation cannot silently select an older complete source"
    );
    Ok(())
}

#[path = "end_record_self_offset.rs"]
mod self_offset;

#[path = "end_record_tall_frame.rs"]
mod tall_frame;
