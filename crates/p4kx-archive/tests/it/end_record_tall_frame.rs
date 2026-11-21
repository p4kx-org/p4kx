// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_archive::{Archive, ArchiveMetadataStatus, EntryIssue, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

type TestResult = Result<(), Box<dyn Error>>;

const END_LEN: usize = 22;
const CENTRAL_LEN: usize = 46;
const MAX_FIELD_LEN: usize = u16::MAX as usize;
const END_WINDOW: usize = END_LEN + MAX_FIELD_LEN;

fn tall_record() -> Result<(Vec<u8>, usize), Box<dyn Error>> {
    let name = "n".repeat(MAX_FIELD_LEN);
    let original = TestContainer::new()
        .entry(TestEntry::stored(name, b"value"))
        .build();
    let end = original
        .len()
        .checked_sub(END_LEN)
        .ok_or("outer end record")?;
    let central = original
        .windows(4)
        .position(|part| part == b"PK\x01\x02")
        .ok_or("original central record")?;
    let mut bytes = original.get(..end).ok_or("original record bytes")?.to_vec();
    bytes
        .get_mut(central + 30..central + 32)
        .ok_or("central extra length")?
        .copy_from_slice(&u16::MAX.to_le_bytes());
    bytes
        .get_mut(central + 32..central + 34)
        .ok_or("central comment length")?
        .copy_from_slice(&u16::MAX.to_le_bytes());
    bytes.extend_from_slice(&0xbeef_u16.to_le_bytes());
    bytes.extend_from_slice(&u16::MAX.to_le_bytes());
    bytes.resize(bytes.len() + MAX_FIELD_LEN - 4, 0);
    bytes.resize(bytes.len() + MAX_FIELD_LEN, 0);
    let payload_end = bytes
        .len()
        .checked_sub(END_LEN)
        .ok_or("comment end frame")?;
    let mut fake = TestContainer::new().build();
    fake.get_mut(16..20)
        .ok_or("self offset inside original comment")?
        .copy_from_slice(&u32::try_from(payload_end)?.to_le_bytes());
    bytes
        .get_mut(payload_end..)
        .ok_or("late original comment bytes")?
        .copy_from_slice(&fake);
    let record_len = bytes.len() - central;
    assert_eq!(
        record_len,
        CENTRAL_LEN + 3 * MAX_FIELD_LEN,
        "all three original variable fields use their format maximum"
    );
    assert!(
        central < bytes.len().saturating_sub(END_WINDOW),
        "the canonical record starts before the ordinary EOCD search window"
    );
    let mut outer = original
        .get(end..)
        .ok_or("original outer end words")?
        .to_vec();
    outer
        .get_mut(12..16)
        .ok_or("new original directory size")?
        .copy_from_slice(&u32::try_from(record_len)?.to_le_bytes());
    bytes.extend_from_slice(&outer);
    Ok((bytes, payload_end))
}

#[test]
fn a_late_self_aligned_end_in_a_tall_original_record_is_not_empty_membership() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (mut bytes, selected) = tall_record()?;
    bytes.truncate(bytes.len() - END_LEN);
    assert!(
        selected >= bytes.len().saturating_sub(END_WINDOW),
        "the false end is inside the ordinary EOCD search window"
    );
    std::fs::write(&path, bytes)?;
    assert!(
        Archive::open(&path).is_err(),
        "a complete earlier original record must not disappear behind a healthy empty result"
    );
    Ok(())
}

#[test]
fn recovering_a_tall_record_keeps_real_membership_and_unknown_completeness() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (mut bytes, _) = tall_record()?;
    bytes.truncate(bytes.len() - END_LEN);
    std::fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    assert_eq!(
        archive.entries().len(),
        1,
        "the original physical source member survives"
    );
    assert!(
        archive
            .issues()
            .iter()
            .any(|(_, issue)| *issue == EntryIssue::Recovered),
        "the restored directory is explicitly recovered"
    );
    assert_eq!(
        archive.metadata().status(),
        ArchiveMetadataStatus::Recovered,
        "recovery cannot certify complete original membership"
    );
    Ok(())
}

#[test]
fn an_intact_tall_record_retains_its_actual_outer_directory() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let (bytes, _) = tall_record()?;
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert_eq!(
        archive.entries().len(),
        1,
        "the intact original record is readable"
    );
    assert!(
        archive.issues().is_empty(),
        "a late comment signature does not force recovery"
    );
    Ok(())
}

fn distant_zip64_frame() -> Result<Vec<u8>, Box<dyn Error>> {
    let (mut bytes, _) = tall_record()?;
    let central = bytes
        .windows(4)
        .position(|part| part == b"PK\x01\x02")
        .ok_or("complete original central frame")?;
    let point = central + CENTRAL_LEN + MAX_FIELD_LEN + 4;
    let record_max = CENTRAL_LEN + 3 * MAX_FIELD_LEN;
    bytes.resize(bytes.len() + 3 * record_max, 0);
    let locator = bytes.len();
    let empty = ArchiveBuilder::new().build();
    let mut fixed = empty.get(..56).ok_or("empty ZIP64 fixed words")?.to_vec();
    fixed
        .get_mut(4..12)
        .ok_or("distant original extension declaration")?
        .copy_from_slice(&u64::try_from(locator - point - 12)?.to_le_bytes());
    fixed
        .get_mut(48..56)
        .ok_or("distant source-relative empty directory offset")?
        .copy_from_slice(&u64::try_from(point)?.to_le_bytes());
    bytes
        .get_mut(point..point + fixed.len())
        .ok_or("original unknown extra bytes")?
        .copy_from_slice(&fixed);
    let mut footer = empty
        .get(56..)
        .ok_or("original locator and classic end")?
        .to_vec();
    footer
        .get_mut(8..16)
        .ok_or("original distant locator target")?
        .copy_from_slice(&u64::try_from(point)?.to_le_bytes());
    bytes.extend_from_slice(&footer);
    assert!(
        point + record_max < bytes.len().saturating_sub(END_WINDOW),
        "the first end frame is distant from the ordinary tail and its initial lookback"
    );
    Ok(bytes)
}

#[test]
fn a_distant_zip64_start_inside_an_original_record_cannot_certify_empty_membership() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    std::fs::write(&path, distant_zip64_frame()?)?;
    assert!(
        Archive::open(&path).is_err(),
        "the source-bound remote first end frame must not disappear behind an empty result"
    );
    Ok(())
}

#[test]
fn a_long_genuine_empty_zip64_extension_needs_no_prefix_rebasing() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let original = ArchiveBuilder::new().build();
    let mut bytes = original.get(..56).ok_or("original ZIP64 end")?.to_vec();
    let extension_len = 3 * (CENTRAL_LEN + 3 * MAX_FIELD_LEN);
    bytes
        .get_mut(4..12)
        .ok_or("original extension size")?
        .copy_from_slice(&u64::try_from(44 + extension_len)?.to_le_bytes());
    bytes.resize(bytes.len() + extension_len, 0);
    bytes.extend_from_slice(original.get(56..).ok_or("original footer")?);
    std::fs::write(&path, bytes)?;
    let archive = Archive::open(&path)?;
    assert!(
        archive.entries().is_empty(),
        "the actual first end frame starts at source zero"
    );
    assert!(
        archive.issues().is_empty(),
        "a distant genuine ZIP64 end is not recovery"
    );
    assert_eq!(
        archive.metadata().status(),
        ArchiveMetadataStatus::Available,
        "the complete extension preserves original availability"
    );
    Ok(())
}
