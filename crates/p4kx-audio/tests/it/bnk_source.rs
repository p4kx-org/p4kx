// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;

use p4kx_audio::{
    BankError, BankReadError, BankSource, HircKind, MediaClass, SectionId, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

#[path = "bnk_source_media_reader.rs"]
mod media_reader;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn bank_source_keeps_duplicate_sections_and_reads_each_original_bkhd() -> TestResult {
    let bytes = duplicate_header_bank();
    assert!(
        matches!(
            Soundbank::parse(bytes.as_slice()),
            Err(BankError::DuplicateSection(SectionId::BKHD))
        ),
        "the decoded reader keeps its recognized-duplicate refusal"
    );
    let mut source = open(&bytes)?;
    let mut cursor = source.section_cursor();
    let first = source.next_section(&mut cursor).ok_or("first BKHD")??;
    assert_eq!(
        (first.ordinal(), first.header_offset(), first.body_len()),
        (0, 0, 44),
        "first original header"
    );
    assert!(!first.is_duplicate_unique(), "first BKHD is not a repeat");
    assert_eq!(source.read_header_for(&first)?.id, 71, "first raw header");

    let second = source.next_section(&mut cursor).ok_or("second BKHD")??;
    assert_eq!(
        (second.ordinal(), second.body_offset(), second.body_len()),
        (1, 60, 12),
        "second physical header"
    );
    assert!(
        second.is_duplicate_unique(),
        "a repeated recognized section remains visible"
    );
    let raw = source.read_header_for(&second)?;
    assert_eq!(
        (raw.version, raw.id, raw.language_id),
        (7, 81, 29),
        "actual unsupported-version raw words"
    );
    assert!(
        !raw.is_supported_version(),
        "raw header does not imply a known decoder version"
    );
    for ordinal in [2, 3] {
        let extra = source
            .next_section(&mut cursor)
            .ok_or("unknown original section")??;
        assert_eq!(
            (extra.id(), extra.ordinal()),
            (SectionId::new(*b"????"), ordinal),
            "unknown tags retain order"
        );
        assert!(!extra.is_duplicate_unique(), "unknown tags can repeat");
    }
    assert!(
        source.next_section(&mut cursor).is_none(),
        "complete original directory"
    );
    Ok(())
}

#[test]
fn bank_source_streams_repeated_didx_ids_and_classifies_only_bounded_media_prefixes() -> TestResult
{
    let bytes = media_bank();
    let legacy = Soundbank::parse(bytes.as_slice())?;
    assert_eq!(
        legacy.wems().map(|wem| wem.id()).collect::<Vec<_>>(),
        [5, 9],
        "legacy selection stays sorted and deduplicated"
    );
    let mut source = open(&bytes)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, index, data] = sections.as_slice() else {
        return Err("one DIDX and one DATA section".into());
    };
    let mut cursor = source.didx(index)?;
    for (ordinal, id, offset, length, class, declared, prefetch) in [
        (0, 9, 0, 4, MediaClass::Impulse, None, false),
        (1, 5, 16, 12, MediaClass::Wem, Some(108), true),
        (2, 5, 32, 4, MediaClass::Midi, None, false),
    ] {
        let entry = cursor.next().ok_or("original DIDX triplet")??;
        assert_eq!(
            (
                entry.section_ordinal(),
                entry.ordinal(),
                entry.triplet_offset()
            ),
            (1, ordinal, 60 + ordinal * 12),
            "physical DIDX coordinate"
        );
        assert_eq!(
            (entry.media_id(), entry.offset(), entry.length()),
            (id, offset, length),
            "original triplet words"
        );
        let media = cursor.inspect_media(&entry, data)?;
        assert_eq!(
            (media.class, media.declared_len, media.prefetch),
            (class, declared, prefetch),
            "actual bounded class and prefetch"
        );
        assert_eq!(
            u32::from(media.prefix_len),
            length,
            "classification reads only the verified available prefix"
        );
        assert_eq!(
            media.range.start,
            data.body_offset() + u64::from(offset),
            "bank-relative data range"
        );
    }
    assert!(
        cursor.next().is_none(),
        "complete triplet inventory keeps both ID occurrences"
    );
    Ok(())
}

#[test]
fn bank_source_keeps_didx_prefix_when_the_tail_or_data_range_is_invalid() -> TestResult {
    let triplet_and_tail = [2, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0xAA];
    let bytes = TestBank::new(1)
        .section(*b"DIDX", triplet_and_tail)
        .section(*b"DATA", [0; 3])
        .bytes();
    let mut source = open(&bytes)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, index, data] = sections.as_slice() else {
        return Err("one original DIDX and DATA section".into());
    };
    let mut cursor = source.didx(index)?;
    let entry = cursor.next().ok_or("valid authored triplet prefix")??;
    assert!(
        matches!(
            cursor.inspect_media(&entry, data),
            Err(BankReadError::Format(BankError::MediaOutsideData {
                id: 2,
                offset: 2,
                end: 4,
                data_len: 3
            }))
        ),
        "outside DATA is diagnosed before reading any media bytes"
    );
    assert!(
        matches!(
            cursor.next(),
            Some(Err(BankReadError::Format(BankError::InvalidSection {
                section: SectionId::DIDX,
                ..
            })))
        ),
        "the incomplete triplet tail has one explicit terminal error"
    );
    assert!(cursor.next().is_none(), "the malformed cursor is terminal");
    Ok(())
}

#[test]
fn bank_source_hirc_keeps_duplicate_ids_raw_type_and_the_exact_trailing_range() -> TestResult {
    let mut bytes = TestBank::new(1)
        .object(TestHircObject::raw(250, 7, [1, 2]))
        .object(TestHircObject::raw(8, 7, [3]))
        .bytes();
    let end = bytes.len() as u64;
    let section_len = bytes.get_mut(56..60).ok_or("HIRC section length")?;
    let declared = u32::try_from(end - 60 + 2)?;
    section_len.copy_from_slice(&declared.to_le_bytes());
    bytes.extend_from_slice(&[0xAA, 0xBB]);
    let mut source = open(&bytes)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, hirc] = sections.as_slice() else {
        return Err("one original HIRC section".into());
    };
    let mut cursor = source.hirc(hirc)?;
    assert_eq!(cursor.object_count(), 2, "authored object count");
    assert_eq!(
        cursor.trailing_range(),
        None,
        "incomplete scan has no trailing range"
    );
    let first = cursor.next().ok_or("first object")??;
    let second = cursor.next().ok_or("second object")??;
    assert_eq!(
        (
            first.ordinal(),
            first.type_code(),
            first.id(),
            first.header_offset(),
            first.body_len()
        ),
        (0, 250, 7, 64, 6),
        "first raw occurrence"
    );
    assert_eq!(
        (
            second.ordinal(),
            second.type_code(),
            second.id(),
            second.header_offset(),
            second.body_len()
        ),
        (1, 8, 7, 75, 5),
        "second same-ID occurrence"
    );
    assert_eq!(
        second.kind(154),
        HircKind::AudioBus,
        "original version applies existing kind interpretation"
    );
    assert_eq!(
        cursor.trailing_range(),
        Some(end..end + 2),
        "remaining bytes are exact and independent of object count"
    );
    Ok(())
}

#[test]
fn bank_source_hirc_reports_the_original_object_extent_outside_its_section() -> TestResult {
    let mut bytes = TestBank::new(1)
        .object(TestHircObject::raw(8, 7, [3]))
        .bytes();
    bytes
        .get_mut(65..69)
        .ok_or("HIRC object length")?
        .copy_from_slice(&20_u32.to_le_bytes());
    let mut source = open(&bytes)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, hirc] = sections.as_slice() else {
        return Err("one HIRC section".into());
    };
    let mut cursor = source.hirc(hirc)?;
    assert!(
        matches!(
            cursor.next(),
            Some(Err(BankReadError::Format(BankError::Truncated {
                section: SectionId::HIRC,
                len: 14,
                needed: 29
            })))
        ),
        "body extent is checked relative to the observed section before reading its ID"
    );
    assert_eq!(
        cursor.trailing_range(),
        None,
        "a failed directory cannot certify trailing bytes"
    );
    Ok(())
}

#[test]
fn bank_source_rejects_foreign_equal_occurrences_and_skips_large_section_bodies() -> TestResult {
    let mut prefix = TestBank::new(1).bytes();
    let large = (768_u32 << 20) + 16;
    prefix.extend_from_slice(b"DATA");
    prefix.extend_from_slice(&large.to_le_bytes());
    let length = prefix.len() as u64 + u64::from(large);
    let mut source = BankSource::open(Cursor::new(prefix.as_slice()), length)?;
    let mut foreign = BankSource::open(Cursor::new(prefix.as_slice()), length)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, data] = sections.as_slice() else {
        return Err("header and virtual large DATA extent".into());
    };
    assert_eq!(
        data.body_end_exclusive(),
        length,
        "fixed reads skip the whole virtual body"
    );
    assert!(
        matches!(
            foreign.read_header_for(header),
            Err(BankReadError::Format(BankError::InvalidSection {
                section: SectionId::BKHD,
                ..
            }))
        ),
        "equal coordinates and bytes cannot substitute the original read owner"
    );
    let mut cursor = source.section_cursor();
    assert!(
        matches!(
            foreign.next_section(&mut cursor),
            Some(Err(BankReadError::Format(BankError::InvalidSection { .. })))
        ),
        "detached cursors stay bound to their originating bank reader"
    );
    Ok(())
}

fn duplicate_header_bank() -> Vec<u8> {
    let repeated = [7_u32, 81, 29]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    TestBank::new(71)
        .section(*b"BKHD", repeated)
        .section(*b"????", [1, 2])
        .section(*b"????", [3])
        .bytes()
}

fn media_bank() -> Vec<u8> {
    TestBank::new(1)
        .wem(9, *b"PLUG")
        .wem(5, *b"RIFF\x64\0\0\0WAVE")
        .wem(5, *b"MThd")
        .bytes()
}

fn open(bytes: &[u8]) -> Result<BankSource<Cursor<&[u8]>>, BankReadError> {
    BankSource::open(Cursor::new(bytes), bytes.len() as u64)
}
