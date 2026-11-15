// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Quirks that leave the rest of a soundbank readable are worked around and reported as warnings.

use std::io::Cursor;

use p4kx_audio::{BankSource, BankWarning, HircKind, HircValue, SectionId, Soundbank};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn a_clean_bank_has_no_warnings() -> TestResult {
    let bytes = TestBank::new(1)
        .wem(5, [1])
        .object(TestHircObject::event(9, &[]))
        .name(1, "Bank")
        .bytes();

    assert_eq!(Soundbank::parse(bytes.as_slice())?.warnings(), []);
    Ok(())
}

#[test]
fn bytes_after_the_last_hirc_object_are_ignored_and_reported() -> TestResult {
    let mut hirc = 1u32.to_le_bytes().to_vec();
    hirc.push(8);
    hirc.extend_from_slice(&4u32.to_le_bytes());
    hirc.extend_from_slice(&77u32.to_le_bytes());
    hirc.extend_from_slice(&[0xAA; 6]);
    let bytes = TestBank::new(1)
        .wem(5, [1, 2, 3])
        .section(*b"HIRC", hirc)
        .bytes();

    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(bank.warnings(), [BankWarning::TrailingHircBytes { len: 6 }]);
    assert_eq!(
        bank.object(77).map(|object| object.kind),
        Some(HircKind::AudioBus),
        "the declared object"
    );
    assert_eq!(bank.wem(5).map(|wem| wem.bytes().len()), Some(3), "WEM 5");
    Ok(())
}

#[test]
fn a_repeated_didx_id_keeps_the_first_entry_in_file_order() -> TestResult {
    let bytes = TestBank::new(1)
        .wem(9, [9])
        .wem(5, [1, 1])
        .wem(5, [2, 2, 2])
        .wem(5, [3])
        .bytes();

    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(
        bank.warnings(),
        [
            BankWarning::DuplicateMedia { id: 5 },
            BankWarning::DuplicateMedia { id: 5 }
        ]
    );
    assert_eq!(
        bank.wem(5).map(|wem| wem.bytes().to_vec()),
        Some(vec![1, 1]),
        "the first WEM 5"
    );
    let ids: Vec<u32> = bank.wems().map(|wem| wem.id()).collect();
    assert_eq!(ids, [5, 9], "ascending, once each");
    Ok(())
}

#[test]
fn eager_media_retains_the_original_triplet_and_data_placement() -> TestResult {
    let index: Vec<u8> = [9_u32, 0, 1, 5, 1, 2, 5, 3, 3]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let bytes = TestBank::new(1)
        .section(*b"DATA", [9, 1, 1, 2, 2, 2])
        .section(*b"INIT", [0; 5])
        .section(*b"DIDX", index)
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let wem = bank.wem(5).ok_or("first eager WEM 5")?;
    assert_eq!(wem.bytes(), [1, 1], "the first physical triplet still wins");
    let placement = wem.source();
    let mut source = BankSource::open(Cursor::new(bytes.as_slice()), bytes.len() as u64)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, data, _, didx] = sections.as_slice() else {
        return Err("expected DATA before DIDX with another section between them".into());
    };
    assert_eq!(data.id(), SectionId::DATA, "physical DATA position");
    assert_eq!(didx.id(), SectionId::DIDX, "physical DIDX position");
    let original = source.didx(didx)?.nth(1).ok_or("original triplet 1")??;
    assert_eq!(
        (
            placement.section_ordinal(),
            placement.entry_ordinal(),
            placement.triplet_offset()
        ),
        (
            original.section_ordinal(),
            original.ordinal(),
            original.triplet_offset()
        ),
        "the positions of the first physical ID 5 survive sorting and deduplication"
    );
    assert_eq!(
        (
            placement.data_section_ordinal(),
            placement.data_body_offset(),
            placement.data_body_length()
        ),
        (data.ordinal(), data.body_offset(), data.body_len()),
        "the checked bounds belong to the actual DATA section"
    );
    assert_eq!(
        placement.byte_range(),
        original.range_in(data)?,
        "the exact WEM payload extent"
    );
    assert_eq!(
        bank.wems().next().map(|media| media.source()),
        Some(placement),
        "the sorted iterator keeps the same physical evidence"
    );
    Ok(())
}

#[test]
fn didx_without_data_leaves_no_media_and_the_hirc_readable() -> TestResult {
    let mut bytes = TestBank::new(1)
        .object(TestHircObject::event(9, &[]))
        .bytes();
    bytes.extend_from_slice(b"DIDX");
    bytes.extend_from_slice(&24u32.to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]);

    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(
        bank.warnings(),
        [BankWarning::MediaWithoutData { entries: 2 }]
    );
    assert_eq!(bank.wems().len(), 0, "no media");
    assert!(
        matches!(
            bank.object(9).map(|object| &object.value),
            Some(HircValue::Event(_))
        ),
        "the event"
    );
    Ok(())
}

#[test]
fn a_non_utf8_bank_name_is_left_out_and_reported() -> TestResult {
    let stid = [
        1, 0, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0, 1, 0xFF, 8, 0, 0, 0, 2, b'O', b'k',
    ];
    let bytes = TestBank::new(1).section(*b"STID", stid).bytes();

    let bank = Soundbank::parse(bytes.as_slice())?;

    assert_eq!(bank.warnings(), [BankWarning::NonUtf8BankName { id: 7 }]);
    assert_eq!(bank.bank_names().collect::<Vec<_>>(), [(8, "Ok")]);
    Ok(())
}

#[test]
fn warnings_describe_the_quirk() {
    assert_eq!(
        BankWarning::TrailingHircBytes { len: 3 }.to_string(),
        "3 bytes follow the last object of the soundbank section HIRC"
    );
}
