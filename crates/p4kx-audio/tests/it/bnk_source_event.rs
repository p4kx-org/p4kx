// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;

use p4kx_audio::{
    BankEventCountEncoding, BankEventRead, BankHircSemantic, BankObservedHeader, BankSource,
    BankSourceSection, HircValue, SectionId, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const EVENT_TYPE: u8 = 4;
const EVENT_ID: u32 = 53;
const RAW_SUFFIX: [u8; 2] = [0xee, 0xa1];

#[test]
fn event_source_preserves_count_width_order_and_original_word_ranges() -> TestResult {
    for (version, count, encoding) in [
        (122, vec![3, 0, 0, 0], BankEventCountEncoding::FixedU32),
        (123, vec![3], BankEventCountEncoding::VariableLength),
    ] {
        let ids = [17_u32, 17, 23];
        let mut body = count.clone();
        body.extend(ids.into_iter().flat_map(u32::to_le_bytes));
        body.extend_from_slice(&RAW_SUFFIX);
        let bytes = bank(version, &body);
        let body_start = u64::try_from(bytes.len() - body.len())?;
        let mut source = open(&bytes)?;
        let (header, section) = parts(&mut source)?;
        let mut hierarchy = source.hirc(&section)?;
        let Some(BankHircSemantic::Event(event)) = hierarchy.next_semantic(&header).transpose()?
        else {
            return Err("actual Event occurrence".into());
        };
        let BankEventRead::Decoded(mut actions) = event.status else {
            return Err("valid Event action list".into());
        };
        assert_eq!(
            actions.encoding(),
            encoding,
            "actual version selects count encoding"
        );
        assert_eq!(actions.declared_count().value, 3, "declared count");
        let end = body_start + u64::try_from(count.len())?;
        assert_eq!(
            actions.declared_count().range,
            body_start..end,
            "exact original count range"
        );
        for (ordinal, id) in ids.into_iter().enumerate() {
            let action = actions
                .next()
                .transpose()?
                .ok_or("original action occurrence")?;
            let offset = end + u64::try_from(ordinal)? * 4;
            assert_eq!(
                action.ordinal,
                u32::try_from(ordinal)?,
                "physical action order"
            );
            assert_eq!(action.action_id.value, id, "original duplicate action ID");
            assert_eq!(
                action.action_id.range,
                offset..offset + 4,
                "exact physical action word"
            );
        }
        assert!(actions.next().is_none(), "complete declared action count");
        let tail = u64::try_from(bytes.len())?;
        assert_eq!(
            actions.trailing_range(),
            Some(tail - 2..tail),
            "unconsumed original suffix"
        );
    }
    Ok(())
}

#[test]
fn malformed_event_reports_the_original_count_prefix_and_legacy_error() -> TestResult {
    for (body, expected_count) in [
        ([0x81].as_slice(), None),
        ([3, 17, 0, 0, 0].as_slice(), Some(3)),
    ] {
        let bytes = bank(154, body);
        let legacy = Soundbank::parse(bytes.as_slice())?;
        let object = legacy.objects().first().ok_or("legacy Event")?;
        let HircValue::Malformed(expected) = &object.value else {
            return Err("legacy parser classifies malformed Event".into());
        };
        let mut source = open(&bytes)?;
        let (header, section) = parts(&mut source)?;
        let mut hierarchy = source.hirc(&section)?;
        let Some(BankHircSemantic::Event(event)) = hierarchy.next_semantic(&header).transpose()?
        else {
            return Err("physical Event survives".into());
        };
        let BankEventRead::Malformed {
            error,
            count,
            count_prefix,
            ..
        } = event.status
        else {
            return Err("original Event count failure".into());
        };
        assert_eq!(&error, expected, "same legacy section-relative error");
        assert_eq!(
            count.map(|word| word.value),
            expected_count,
            "only a fully read count is present"
        );
        let start = u64::try_from(bytes.len() - body.len())?;
        assert_eq!(
            count_prefix,
            start..start + 1,
            "exact successfully read count prefix"
        );
        assert!(
            hierarchy.next_semantic(&header).is_none(),
            "no fabricated extra objects"
        );
    }
    Ok(())
}

#[test]
fn event_semantic_cursor_rejects_foreign_header_before_consuming_the_object() -> TestResult {
    let bytes = bank(154, &[0]);
    let mut own = open(&bytes)?;
    let mut other = open(&bytes)?;
    let (own_header, section) = parts(&mut own)?;
    let (foreign, _) = parts(&mut other)?;
    let mut hierarchy = own.hirc(&section)?;
    assert!(
        hierarchy
            .next_semantic(&foreign)
            .is_some_and(|result| result.is_err()),
        "equal version from a different bank read is foreign"
    );
    let Some(BankHircSemantic::Event(event)) = hierarchy.next_semantic(&own_header).transpose()?
    else {
        return Err("first original object after foreign rejection".into());
    };
    assert_eq!(
        event.object.ordinal(),
        0,
        "foreign proof did not consume the cursor"
    );
    Ok(())
}

fn bank(version: u32, body: &[u8]) -> Vec<u8> {
    TestBank::new(71)
        .version(version)
        .object(TestHircObject::raw(EVENT_TYPE, EVENT_ID, body))
        .bytes()
}

fn open(bytes: &[u8]) -> TestResult<BankSource<Cursor<&[u8]>>> {
    Ok(BankSource::open(
        Cursor::new(bytes),
        u64::try_from(bytes.len())?,
    )?)
}

fn parts(
    source: &mut BankSource<Cursor<&[u8]>>,
) -> TestResult<(BankObservedHeader, BankSourceSection)> {
    let mut sections = source.section_cursor();
    let header = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let hierarchy = source
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    assert_eq!(hierarchy.id(), SectionId::HIRC, "fixture section order");
    Ok((source.read_observed_header_for(&header)?, hierarchy))
}
