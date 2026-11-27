// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Cursor, ErrorKind, Read, Seek, SeekFrom};

use p4kx_audio::{
    BankDidx, BankError, BankReadError, BankSource, BankSourceSection, SectionId, WemSource,
};
use p4kx_testkit::{TestBank, TestCodec, TestWem};

use super::{TestResult, open};

#[test]
fn didx_media_reader_is_relative_to_the_exact_range_and_resumes_the_cursor() -> TestResult {
    let wem = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &[1, -2, 3]).bytes();
    let prefix = b"before";
    let suffix = b"after";
    let length = u32::try_from(wem.len())?;
    let offset = u32::try_from(prefix.len())?;
    let data = [prefix.as_slice(), wem.as_slice(), suffix.as_slice()].concat();
    let empty = u32::try_from(data.len())?;
    let bank = TestBank::new(1)
        .section(*b"DIDX", words([17, offset, length, 17, empty, 0]))
        .section(*b"DATA", data)
        .section(*b"TAIL", b"outside DATA".as_slice())
        .bytes();
    let mut source = open(&bank)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, index, data, _] = sections.as_slice() else {
        return Err("one physical DIDX, one DATA and one following section".into());
    };
    assert_eq!(data.id(), SectionId::DATA, "the selected DATA occurrence");
    let mut cursor = source.didx(index)?;
    let media = cursor.next().ok_or("original nonempty triplet")??;
    let start = usize::try_from(data.body_offset())? + prefix.len();
    let end = start + wem.len();
    assert_eq!(
        bank.get(start..end),
        Some(wem.as_slice()),
        "the original range is the WEM, surrounded by independent sentinels"
    );
    assert_eq!(
        media.range_in(data)?,
        u64::try_from(start)?..u64::try_from(end)?,
        "the actual DIDX and DATA fields resolve that exact half-open range"
    );

    {
        let mut reader = cursor.media_reader(&media, data)?;
        check_window(&mut reader, &wem)?;
        let mut source = WemSource::open(reader, u64::from(length))?;
        let metadata = source.read_metadata()?;
        assert_eq!(metadata.channels(), 1, "the original PCM channels");
        assert_eq!(
            metadata.sample_count(),
            Some(3),
            "the actual PCM frame count"
        );
        assert_eq!(source.riff_end(), u64::from(length), "RIFF-local extent");
    }
    check_empty_occurrence(&mut cursor, data)
}

fn check_empty_occurrence<R: Read + Seek>(
    cursor: &mut BankDidx<'_, R>,
    data: &BankSourceSection,
) -> TestResult {
    let next = cursor.next().ok_or("next actual DIDX triplet")??;
    assert_eq!(
        (next.ordinal(), next.media_id(), next.length()),
        (1, 17, 0),
        "the physical cursor resumes at the next occurrence of the same ID"
    );
    {
        let mut reader = cursor.media_reader(&next, data)?;
        assert_eq!(
            reader.seek(SeekFrom::End(0))?,
            0,
            "zero-length window extent"
        );
        assert_eq!(
            reader.read(&mut [0; 1])?,
            0,
            "empty window does not read the next section"
        );
    }
    assert!(
        cursor.next().is_none(),
        "both original triplets were visited"
    );
    Ok(())
}

fn check_window(reader: &mut (impl Read + Seek), wem: &[u8]) -> TestResult {
    let length = u64::try_from(wem.len())?;
    let mut signature = [0; 4];
    reader.read_exact(&mut signature)?;
    assert_eq!(
        &signature, b"RIFF",
        "byte zero maps to the selected media start"
    );
    assert_eq!(
        reader.seek(SeekFrom::Current(-2))?,
        2,
        "current seek is local"
    );
    assert_eq!(
        reader.seek(SeekFrom::End(-1))?,
        length - 1,
        "end seek is local"
    );
    let mut tail = [0xff; 4];
    assert_eq!(
        reader.read(&mut tail)?,
        1,
        "reading stops before the following sentinel"
    );
    let last = *wem.last().ok_or("the actual final WEM byte")?;
    assert_eq!(
        tail,
        [last, 0xff, 0xff, 0xff],
        "no adjacent data entered the buffer"
    );
    assert_eq!(
        reader.read(&mut tail)?,
        0,
        "the aperture ends at the selected byte"
    );
    check_rejected_seeks(reader, length)
}

fn check_rejected_seeks(reader: &mut impl Seek, length: u64) -> TestResult {
    for invalid in [
        SeekFrom::Start(u64::MAX),
        SeekFrom::End(1),
        SeekFrom::Current(i64::MIN),
    ] {
        let error = reader
            .seek(invalid)
            .err()
            .ok_or("seek outside the media window")?;
        assert_eq!(
            error.kind(),
            ErrorKind::InvalidInput,
            "the invalid seek is explicit"
        );
        assert_eq!(
            reader.stream_position()?,
            length,
            "a rejected seek leaves position unchanged"
        );
    }
    reader.rewind()?;
    assert_eq!(
        reader.stream_position()?,
        0,
        "rewind to media-relative byte zero"
    );
    Ok(())
}

#[test]
fn didx_media_reader_refuses_foreign_and_outside_data_occurrences() -> TestResult {
    let bank = TestBank::new(1)
        .section(*b"DIDX", words([1, 1, 2, 2, 3, 2]))
        .section(*b"DIDX", words([1, 1, 2]))
        .section(*b"DATA", [0x11, 0x22, 0x33, 0x44])
        .bytes();
    let mut source = open(&bank)?;
    let mut foreign = open(&bank)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let other_sections = foreign.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, index, wrong_index, data] = sections.as_slice() else {
        return Err("two physical DIDX sections and one DATA".into());
    };
    let [_, foreign_index, _, foreign_data] = other_sections.as_slice() else {
        return Err("identical sections from a different bank read".into());
    };
    let wrong = source
        .didx(wrong_index)?
        .next()
        .ok_or("wrong DIDX occurrence")??;
    let foreign_entry = foreign
        .didx(foreign_index)?
        .next()
        .ok_or("foreign DIDX occurrence")??;
    let mut cursor = source.didx(index)?;
    let entry = cursor.next().ok_or("selected DIDX occurrence")??;
    assert_eq!(
        (entry.ordinal(), entry.triplet_offset()),
        (foreign_entry.ordinal(), foreign_entry.triplet_offset()),
        "the equal bytes and position do not establish common authority"
    );
    for rejected in [&wrong, &foreign_entry] {
        assert_invalid_section(&cursor.media_reader(rejected, data), SectionId::DIDX);
    }
    for rejected in [index, foreign_data] {
        assert_invalid_section(&cursor.media_reader(&entry, rejected), SectionId::DATA);
    }
    {
        let mut reader = cursor.media_reader(&entry, data)?;
        let mut actual = [0; 2];
        reader.read_exact(&mut actual)?;
        assert_eq!(
            actual,
            [0x22, 0x33],
            "the original owned control still reads"
        );
    }
    let outside = cursor.next().ok_or("authored outside-DATA triplet")??;
    assert!(
        matches!(
            cursor.media_reader(&outside, data),
            Err(BankReadError::Format(BankError::MediaOutsideData {
                id: 2,
                offset: 3,
                end: 5,
                data_len: 4,
            }))
        ),
        "the original outside-DATA extent is refused"
    );
    Ok(())
}

fn assert_invalid_section<T>(result: &Result<T, BankReadError>, expected: SectionId) {
    assert!(
        matches!(
            result,
            Err(BankReadError::Format(BankError::InvalidSection { section, .. }))
                if *section == expected
        ),
        "a foreign or wrong {expected} occurrence cannot open the window"
    );
}

#[test]
fn didx_media_reader_preserves_physical_read_failure_inside_a_valid_extent() -> TestResult {
    let mut bank = TestBank::new(1)
        .section(*b"DIDX", words([1, 1, 3]))
        .section(*b"DATA", [0x11, 0x22, 0x33, 0x44])
        .bytes();
    let declared_length = u64::try_from(bank.len())?;
    assert_eq!(
        bank.pop(),
        Some(0x44),
        "one physical byte is missing from DATA"
    );
    let mut source = BankSource::open(Cursor::new(bank), declared_length)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [_, index, data] = sections.as_slice() else {
        return Err("one original DIDX and DATA".into());
    };
    let mut cursor = source.didx(index)?;
    let entry = cursor.next().ok_or("range-valid original triplet")??;
    assert_eq!(
        entry.range_in(data)?.end,
        declared_length,
        "the authored range is valid against the verified logical extent"
    );
    let mut reader = cursor.media_reader(&entry, data)?;
    let cause = reader
        .read_exact(&mut [0; 3])
        .err()
        .ok_or("original source ended inside the declared range")?;
    assert_eq!(
        cause.kind(),
        ErrorKind::UnexpectedEof,
        "physical EOF survives"
    );
    assert_eq!(
        reader.stream_position()?,
        0,
        "failed reads do not advance the window"
    );
    Ok(())
}

fn words<const N: usize>(values: [u32; N]) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}
