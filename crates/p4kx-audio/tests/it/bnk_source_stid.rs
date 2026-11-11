// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Cursor;

use p4kx_audio::{
    BankError, BankReadError, BankSource, BankSourceSection, BankStidName, BankWarning, SectionId,
    Soundbank,
};
use p4kx_testkit::TestBank;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn bank_source_stid_retains_duplicate_ids_invalid_bytes_and_empty_names_in_order() -> TestResult {
    let mut body = stid_body(17, 4);
    append_name(&mut body, 7, 2, b"Ab");
    append_name(&mut body, 7, 2, &[0xFF, 0]);
    append_name(&mut body, 7, 2, "É".as_bytes());
    append_name(&mut body, 8, 0, b"");
    body.extend_from_slice(&[0xAA, 0xBB]);
    let bytes = TestBank::new(1).section(*b"STID", body).bytes();
    let mut source = open(&bytes)?;
    let section = first_stid(&mut source)?;
    let mut names = source.stid(&section)?;

    assert_eq!(names.string_type(), 17, "the original STID type word");
    assert_eq!(names.name_count(), 4, "the authored STID name count");
    assert_eq!(names.trailing_range(), None, "the names have not been read");

    let first = names.next().ok_or("first STID name")??;
    check_name(&first, 0, 68, 7, b"Ab");
    assert_eq!(first.utf8(), Ok("Ab"), "the first decoded name");

    let second = names.next().ok_or("second STID name")??;
    check_name(&second, 1, 75, 7, &[0xFF, 0]);
    assert!(
        second.utf8().is_err(),
        "invalid UTF-8 stays distinguishable"
    );

    let third = names.next().ok_or("third STID name")??;
    check_name(&third, 2, 82, 7, "É".as_bytes());
    assert_eq!(third.utf8(), Ok("É"), "two source bytes, one character");

    let fourth = names.next().ok_or("fourth STID name")??;
    check_name(&fourth, 3, 89, 8, b"");
    assert_eq!(fourth.utf8(), Ok(""), "an empty authored name");
    assert!(names.next().is_none(), "one item per physical occurrence");
    assert_eq!(names.trailing_range(), Some(94..96), "the original tail");
    assert_legacy_names(&bytes)?;
    Ok(())
}

#[test]
fn bank_source_stid_keeps_the_readable_prefix_of_an_impossible_count() -> TestResult {
    let mut body = stid_body(1, u32::MAX);
    append_name(&mut body, 4, 0, b"");
    let bytes = TestBank::new(1).section(*b"STID", body).bytes();
    let mut source = open(&bytes)?;
    let section = first_stid(&mut source)?;
    let mut names = source.stid(&section)?;

    assert_eq!(names.name_count(), u32::MAX, "the stored count");
    let first = names.next().ok_or("readable STID prefix")??;
    check_name(&first, 0, 68, 4, b"");
    assert!(
        matches!(
            names.next(),
            Some(Err(BankReadError::Format(BankError::Truncated {
                section: SectionId::STID,
                len: 13,
                needed: 17,
            })))
        ),
        "the next missing ID word ends the streaming scan"
    );
    assert!(names.next().is_none(), "a malformed count is terminal");
    assert_eq!(names.trailing_range(), None, "the scan is incomplete");
    assert!(
        matches!(
            Soundbank::parse(bytes.as_slice()),
            Err(BankError::InvalidSection {
                section: SectionId::STID,
                ..
            })
        ),
        "the compatibility parser rejects an impossible count up front"
    );
    Ok(())
}

#[test]
fn bank_source_stid_bounds_the_name_by_its_section() -> TestResult {
    let mut body = stid_body(3, 2);
    append_name(&mut body, 4, 0, b"");
    append_name(&mut body, 5, 4, b"X");
    let bytes = TestBank::new(1)
        .section(*b"STID", body)
        .section(*b"NEXT", *b"later")
        .bytes();
    let mut source = open(&bytes)?;
    let section = first_stid(&mut source)?;
    let mut names = source.stid(&section)?;
    let first = names.next().ok_or("readable prefix name")??;

    check_name(&first, 0, 68, 4, b"");
    assert!(
        matches!(
            names.next(),
            Some(Err(BankReadError::Format(BankError::Truncated {
                section: SectionId::STID,
                len: 19,
                needed: 22,
            })))
        ),
        "a declared name cannot borrow bytes from the following section"
    );
    assert!(names.next().is_none(), "a malformed name is terminal");
    assert_eq!(names.trailing_range(), None, "the scan is incomplete");
    assert_eq!(
        Soundbank::parse(bytes.as_slice()).err(),
        Some(BankError::Truncated {
            section: SectionId::STID,
            len: 19,
            needed: 22,
        }),
        "compatibility reports the same name extent"
    );
    Ok(())
}

#[test]
fn bank_source_stid_visits_each_physical_section_and_rejects_a_foreign_owner() -> TestResult {
    let mut body = stid_body(9, 1);
    append_name(&mut body, 7, 1, b"A");
    let mut second_body = stid_body(10, 0);
    second_body.push(0xAB);
    let bytes = TestBank::new(1)
        .section(*b"STID", body)
        .section(*b"STID", second_body)
        .bytes();
    let mut source = open(&bytes)?;
    let mut sections = source.section_cursor();
    let header = source.next_section(&mut sections).ok_or("BKHD section")??;
    let first = source.next_section(&mut sections).ok_or("first STID")??;
    let second = source.next_section(&mut sections).ok_or("second STID")??;

    assert!(!first.is_duplicate_unique(), "the first STID occurrence");
    assert!(second.is_duplicate_unique(), "the second STID occurrence");
    assert_eq!(source.stid(&first)?.name_count(), 1, "the first count");
    let names = source.stid(&second)?;
    assert_eq!(names.string_type(), 10, "the second STID type word");
    assert_eq!(names.name_count(), 0, "the second STID is empty");
    assert_eq!(names.trailing_range(), Some(90..91), "its own tail");
    assert!(source.stid(&header).is_err(), "the original tag is checked");
    let mut foreign = open(&bytes)?;
    assert!(foreign.stid(&first).is_err(), "the source owner is checked");
    assert_eq!(
        Soundbank::parse(bytes.as_slice()).err(),
        Some(BankError::DuplicateSection(SectionId::STID)),
        "the compatibility parser still requires STID to be unique"
    );
    Ok(())
}

fn assert_legacy_names(bytes: &[u8]) -> TestResult {
    let bank = Soundbank::parse(bytes)?;
    assert_eq!(
        bank.bank_names().collect::<Vec<_>>(),
        [(7, "Ab"), (7, "É"), (8, "")],
        "compatibility preserves readable names in original order"
    );
    assert_eq!(
        bank.warnings(),
        [BankWarning::NonUtf8BankName { id: 7 }],
        "compatibility warns about the one unreadable name"
    );
    Ok(())
}

fn check_name(name: &BankStidName, ordinal: u64, header_offset: u64, id: u32, bytes: &[u8]) {
    assert_eq!(name.ordinal(), ordinal, "original name ordinal");
    assert_eq!(name.section_ordinal(), 1, "original STID section ordinal");
    assert_eq!(name.header_offset(), header_offset, "original ID position");
    assert_eq!(name.id(), id, "original ID word");
    assert_eq!(usize::from(name.name_len()), bytes.len(), "original length");
    let start = header_offset + 5;
    assert_eq!(
        name.name_range(),
        start..start + bytes.len() as u64,
        "original name byte range"
    );
    assert_eq!(name.bytes(), bytes, "original name bytes");
}

fn stid_body(string_type: u32, count: u32) -> Vec<u8> {
    let mut body = string_type.to_le_bytes().to_vec();
    body.extend_from_slice(&count.to_le_bytes());
    body
}

fn append_name(body: &mut Vec<u8>, id: u32, declared_len: u8, bytes: &[u8]) {
    body.extend_from_slice(&id.to_le_bytes());
    body.push(declared_len);
    body.extend_from_slice(bytes);
}

fn first_stid(source: &mut BankSource<Cursor<&[u8]>>) -> Result<BankSourceSection, BankReadError> {
    source
        .sections()
        .nth(1)
        .unwrap_or_else(|| Err(BankError::NotSoundbank.into()))
}

fn open(bytes: &[u8]) -> Result<BankSource<Cursor<&[u8]>>, BankReadError> {
    BankSource::open(Cursor::new(bytes), bytes.len() as u64)
}
