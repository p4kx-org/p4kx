// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;
use std::ops::Range;

use p4kx_audio::{
    BankError, BankSource, BankStateRead, HircKind, HircLayout, HircValue, SectionId, Soundbank,
    StateObject,
};
use p4kx_testkit::{TestBank, TestHircObject};

type TestResult = Result<(), Box<dyn Error>>;

const STATE_ID: u32 = 0x1234_5678;
const STATE_TYPE: u8 = 1;
const PROPERTY_IDS: [u16; 3] = [7, 0xF302, 7];
const VALUE_BITS: [u32; 3] = [0xBF80_0000, 0x7FC1_2345, 0x8000_0000];

#[test]
fn state_layout_128_through_145_decodes_the_authored_planar_properties() -> TestResult {
    let payload = state_body(&PROPERTY_IDS, &VALUE_BITS)?;
    for version in [128, 145] {
        let bytes = state_bank(version, &payload);
        let bank = Soundbank::parse(bytes.as_slice())?;
        let object = bank.objects().first().ok_or("original State object")?;
        assert_eq!(
            (object.kind, object.id),
            (HircKind::State, STATE_ID),
            "physical type and ID"
        );
        assert_eq!(
            bank.object_body(object).get(size_of::<u32>()..),
            Some(payload.as_slice()),
            "authored State payload including repeated and unknown IDs"
        );
        assert_eq!(
            object.layout(),
            HircLayout::Decoded,
            "supported State version {version}"
        );
    }
    Ok(())
}

#[test]
fn standalone_state_of_unproved_version_remains_raw_and_unsupported() -> TestResult {
    let payload = state_body(&PROPERTY_IDS, &VALUE_BITS)?;
    for version in [127, 146, 154] {
        let bytes = state_bank(version, &payload);
        let bank = Soundbank::parse(bytes.as_slice())?;
        let object = bank.objects().first().ok_or("original State object")?;
        assert_eq!(
            bank.object_body(object).get(size_of::<u32>()..),
            Some(payload.as_slice()),
            "unchanged raw payload"
        );
        assert_eq!(
            object.layout(),
            HircLayout::Unsupported,
            "standalone State version {version}"
        );
    }
    Ok(())
}

#[test]
fn a_truncated_state_preserves_its_body_and_does_not_hide_the_following_object() -> TestResult {
    let payload = state_body(&PROPERTY_IDS, &VALUE_BITS[..2])?;
    let bytes = TestBank::new(15)
        .version(145)
        .object(TestHircObject::raw(
            STATE_TYPE,
            STATE_ID,
            payload.as_slice(),
        ))
        .object(TestHircObject::event(8, &[9]))
        .bytes();
    let bank = Soundbank::parse(bytes.as_slice())?;
    let [state, event] = bank.objects() else {
        return Err("two physical objects".into());
    };
    assert_eq!(
        bank.object_body(state).get(size_of::<u32>()..),
        Some(payload.as_slice()),
        "original truncated bytes survive"
    );
    assert_eq!(
        state.layout(),
        HircLayout::Malformed,
        "missing State value word"
    );
    assert_eq!(
        event.layout(),
        HircLayout::Decoded,
        "next object still decodes"
    );
    Ok(())
}

#[test]
fn slice_and_seek_state_readers_agree_on_raw_words_and_original_spans() -> TestResult {
    let mut payload = state_body(&PROPERTY_IDS, &VALUE_BITS)?;
    let opaque = [0xEE, 0xA5];
    payload.extend(opaque);
    let bytes = state_bank(145, &payload);
    let bank = Soundbank::parse(bytes.as_slice())?;
    let object = bank.objects().first().ok_or("original State object")?;
    let HircValue::State(state) = &object.value else {
        return Err("typed State properties".into());
    };
    check_original_words(&bytes, state)?;
    assert_eq!(
        bytes_in(&bytes, &state.trailing_range)?,
        &opaque,
        "unparsed original body suffix"
    );
    let mut reader = BankSource::open(Cursor::new(bytes.as_slice()), u64::try_from(bytes.len())?)?;
    let sections = reader.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, hirc] = sections.as_slice() else {
        return Err("two original sections".into());
    };
    let header = reader.read_observed_header_for(header)?;
    let mut hirc = reader.hirc(hirc)?;
    let read = hirc
        .next_state(&header)
        .ok_or("physical seek occurrence")??;
    let BankStateRead::Decoded(seek) = read.status else {
        return Err("seek State properties".into());
    };
    assert_eq!(
        &seek,
        state.as_ref(),
        "both readers retain identical absolute positions and raw values"
    );
    assert_eq!(read.object.id(), STATE_ID, "owner-bound original object ID");
    assert!(
        hirc.next_state(&header).is_none(),
        "one physical occurrence"
    );
    Ok(())
}

#[test]
fn state_count_uses_the_full_encoded_range_and_validates_truncation() -> TestResult {
    let count = usize::from(u16::MAX);
    let ids = vec![0xFFFE; count];
    let bits = vec![0x7F80_0000; count];
    let full = state_body(&ids, &bits)?;
    let bank_bytes = state_bank(128, &full);
    let bank = Soundbank::parse(bank_bytes.as_slice())?;
    let object = bank.objects().first().ok_or("full-size State")?;
    let HircValue::State(state) = &object.value else {
        return Err("full-size State decoded".into());
    };
    assert_eq!(
        state.properties.entries().len(),
        count,
        "every representable property fits the reader"
    );
    let last = state
        .properties
        .entries()
        .last()
        .ok_or("last authored State property")?;
    assert_eq!(
        (last.id.value, last.value_bits.value),
        (0xFFFE, 0x7F80_0000),
        "last original words"
    );
    let missing = u16::MAX.to_le_bytes();
    let short_bytes = state_bank(145, &missing);
    let bank = Soundbank::parse(short_bytes.as_slice())?;
    let error = bank
        .objects()
        .first()
        .and_then(|object| object.error())
        .ok_or("bounded count error")?;
    assert_eq!(
        error,
        &BankError::Truncated {
            section: SectionId::HIRC,
            len: 0,
            needed: 393_210
        },
        "declared planes are checked before materialization"
    );
    Ok(())
}

fn check_original_words(bytes: &[u8], state: &StateObject) -> TestResult {
    let count = state.properties.count();
    assert_eq!(count.value, 3, "authored count");
    assert_eq!(
        bytes_in(bytes, &count.range)?,
        &3_u16.to_le_bytes(),
        "exact count word"
    );
    let properties = state.properties.entries().collect::<Vec<_>>();
    for (property, (id, bits)) in properties
        .iter()
        .zip(PROPERTY_IDS.into_iter().zip(VALUE_BITS))
    {
        assert_eq!(
            (property.id.value, property.value_bits.value),
            (id, bits),
            "ordered properties retain unknown IDs and NaN payload"
        );
        assert_eq!(
            bytes_in(bytes, &property.id.range)?,
            &id.to_le_bytes(),
            "actual planar ID word"
        );
        assert_eq!(
            bytes_in(bytes, &property.value_bits.range)?,
            &bits.to_le_bytes(),
            "actual planar binary32 word"
        );
    }
    let nan = properties.get(1).ok_or("NaN property")?;
    let negative_zero = properties.get(2).ok_or("signed zero property")?;
    assert!(
        nan.value().is_nan(),
        "the typed representation preserves a NaN"
    );
    assert_eq!(
        negative_zero.value().to_bits(),
        0x8000_0000,
        "negative zero is still distinct"
    );
    let consumed = state.properties.consumed_range();
    assert_eq!(
        consumed.end, state.trailing_range.start,
        "consumed extent ends at opaque suffix"
    );
    Ok(())
}

fn bytes_in<'input>(
    input: &'input [u8],
    range: &Range<u64>,
) -> Result<&'input [u8], Box<dyn Error>> {
    input
        .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
        .ok_or_else(|| "original field span".into())
}

fn state_body(ids: &[u16], bits: &[u32]) -> Result<Vec<u8>, std::num::TryFromIntError> {
    let mut bytes = u16::try_from(ids.len())?.to_le_bytes().to_vec();
    bytes.extend(ids.iter().copied().flat_map(u16::to_le_bytes));
    bytes.extend(bits.iter().copied().flat_map(u32::to_le_bytes));
    Ok(bytes)
}

fn state_bank(version: u32, payload: &[u8]) -> Vec<u8> {
    TestBank::new(15)
        .version(version)
        .object(TestHircObject::raw(STATE_TYPE, STATE_ID, payload))
        .bytes()
}
