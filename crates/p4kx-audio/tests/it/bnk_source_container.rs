// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;

use p4kx_audio::{
    BankContainerRead, BankHirc, BankHircSemantic, BankObservedHeader, BankSource, ContainerKind,
    HircValue, SectionId, Soundbank,
};
use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, TestNodeBase};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const IDS: [u32; 3] = [0x9814_5678, 0x9814_5678, 0x3255_9187];
const OBJECT_ID: u32 = 67;
const BLEND_TYPE: u8 = 9;
const WORD_LEN: u64 = 4;

#[test]
fn seekable_container_preserves_order_words_and_all_slice_container_meanings() -> TestResult {
    for (test_kind, expected) in [
        (TestContainerKind::Random, ContainerKind::Random),
        (TestContainerKind::Sequence, ContainerKind::Sequence),
        (TestContainerKind::Switch, ContainerKind::Switch),
        (TestContainerKind::Blend, ContainerKind::Blend),
    ] {
        let object = TestHircObject::container(test_kind, OBJECT_ID, &IDS, TestNodeBase::Full);
        let bytes = TestBank::new(1).object(object).bytes();
        let legacy = Soundbank::parse(&bytes)?;
        let HircValue::Container(original) =
            &legacy.objects().first().ok_or("original container")?.value
        else {
            return Err("fixture must decode through the slice reader".into());
        };
        let mut bank = open(&bytes)?;
        let (header, mut hirc) = parts(&mut bank)?;
        let Some(BankHircSemantic::Container(container)) =
            hirc.next_semantic(&header).transpose()?
        else {
            return Err("typed physical container".into());
        };
        assert!(matches!(container.status, BankContainerRead::Decoded));
        assert_eq!(container.fields.kind, Some(expected));
        assert_eq!(original.kind, expected);
        let parent = container
            .fields
            .node
            .parent_id
            .as_ref()
            .ok_or("parent word")?;
        assert_eq!(parent.value, original.parent_id);
        assert_eq!(word_at(&bytes, parent.range.start)?, parent.value);
        check_children(
            &bytes,
            container.children.ok_or("admitted child span")?,
            container
                .fields
                .child_count
                .as_ref()
                .ok_or("authored count")?
                .range
                .end,
        )?;
        assert_eq!(original.children, IDS);
    }
    Ok(())
}

#[test]
fn seekable_container_keeps_admitted_children_after_a_malformed_suffix() -> TestResult {
    let object = TestHircObject::container(
        TestContainerKind::Blend,
        OBJECT_ID,
        &IDS,
        TestNodeBase::Plain,
    );
    let valid = TestBank::new(1).object(object).bytes();
    let legacy = Soundbank::parse(&valid)?;
    let original = legacy.object_body(legacy.objects().first().ok_or("original blend")?);
    let body = original
        .get(4..original.len() - 1)
        .ok_or("omit the final original flag")?;
    let bytes = TestBank::new(1)
        .object(TestHircObject::raw(BLEND_TYPE, OBJECT_ID, body))
        .object(TestHircObject::raw(0xa0, 88, Vec::new()))
        .bytes();
    let legacy = Soundbank::parse(&bytes)?;
    let HircValue::Malformed(error) = &legacy
        .objects()
        .first()
        .ok_or("original malformed blend")?
        .value
    else {
        return Err("legacy parser must see the same suffix failure".into());
    };
    let mut bank = open(&bytes)?;
    let (header, mut hirc) = parts(&mut bank)?;
    let Some(BankHircSemantic::Container(container)) = hirc.next_semantic(&header).transpose()?
    else {
        return Err("malformed typed container".into());
    };
    let BankContainerRead::Malformed(observed) = container.status else {
        return Err("original suffix failure".into());
    };
    assert_eq!(&observed, error);
    let range = container
        .fields
        .children
        .ok_or("child span before malformed suffix")?;
    check_children(
        &bytes,
        container.children.ok_or("valid child reader")?,
        range.start,
    )?;
    let Some(BankHircSemantic::Raw(next)) = hirc.next_semantic(&header).transpose()? else {
        return Err("adjacent object remains physically readable".into());
    };
    assert_eq!(next.id(), 88);
    Ok(())
}

#[test]
fn impossible_container_child_count_keeps_the_word_and_never_creates_a_cursor() -> TestResult {
    let object = TestHircObject::container(
        TestContainerKind::Blend,
        OBJECT_ID,
        &IDS,
        TestNodeBase::Plain,
    );
    let bytes = TestBank::new(1).object(object).bytes();
    let legacy = Soundbank::parse(&bytes)?;
    let object = legacy
        .objects()
        .first()
        .ok_or("original fixture container")?;
    let mut body = legacy
        .object_body(object)
        .get(4..)
        .ok_or("body after object ID")?
        .to_vec();
    let sequence: Vec<_> = IDS.into_iter().flat_map(u32::to_le_bytes).collect();
    let children = body
        .windows(sequence.len())
        .position(|word| word == sequence.as_slice())
        .ok_or("fixture child sequence")?;
    let count = children
        .checked_sub(4)
        .ok_or("authored count before sequence")?;
    body.get_mut(count..children)
        .ok_or("fixture count extent")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    let bytes = TestBank::new(1)
        .object(TestHircObject::raw(BLEND_TYPE, OBJECT_ID, body))
        .bytes();
    let mut bank = open(&bytes)?;
    let (header, mut hirc) = parts(&mut bank)?;
    let Some(BankHircSemantic::Container(container)) = hirc.next_semantic(&header).transpose()?
    else {
        return Err("physical container with malformed count".into());
    };
    assert!(matches!(container.status, BankContainerRead::Malformed(_)));
    assert_eq!(
        container.fields.child_count.map(|count| count.value),
        Some(u32::MAX)
    );
    assert!(container.fields.children.is_none());
    assert!(container.children.is_none());
    Ok(())
}

fn check_children(
    bytes: &[u8],
    children: p4kx_audio::BankContainerChildren<'_, Cursor<&[u8]>>,
    start: u64,
) -> TestResult {
    let mut children = children;
    for (index, id) in IDS.into_iter().enumerate() {
        let child = children.next().transpose()?.ok_or("original child word")?;
        let start = start + u64::try_from(index)? * WORD_LEN;
        assert_eq!(child.ordinal, u32::try_from(index)?, "physical child order");
        assert_eq!(child.child_id.value, id, "original child ID");
        assert_eq!(
            child.child_id.range,
            start..start + WORD_LEN,
            "original child word extent"
        );
        assert_eq!(
            word_at(bytes, start)?,
            id,
            "child ID at its recorded byte position"
        );
    }
    assert!(
        children.next().is_none(),
        "declared child range consumed exactly"
    );
    Ok(())
}

fn word_at(bytes: &[u8], start: u64) -> TestResult<u32> {
    let start = usize::try_from(start)?;
    let word = bytes
        .get(start..)
        .and_then(<[u8]>::first_chunk)
        .copied()
        .ok_or("original word")?;
    Ok(u32::from_le_bytes(word))
}

fn open(bytes: &[u8]) -> TestResult<BankSource<Cursor<&[u8]>>> {
    Ok(BankSource::open(
        Cursor::new(bytes),
        u64::try_from(bytes.len())?,
    )?)
}

fn parts<'input, 'source>(
    bank: &'input mut BankSource<Cursor<&'source [u8]>>,
) -> TestResult<(BankObservedHeader, BankHirc<'input, Cursor<&'source [u8]>>)> {
    let mut sections = bank.section_cursor();
    let header = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("BKHD")?;
    let hirc = bank
        .next_section(&mut sections)
        .transpose()?
        .ok_or("HIRC")?;
    assert_eq!(
        header.id(),
        SectionId::BKHD,
        "the original version-bearing section"
    );
    assert_eq!(
        hirc.id(),
        SectionId::HIRC,
        "the original object directory section"
    );
    Ok((bank.read_observed_header_for(&header)?, bank.hirc(&hirc)?))
}
