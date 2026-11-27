// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;
use std::ops::Range;

use p4kx_audio::{
    BankError, BankHirc, BankHircRtpc, BankObservedHeader, BankRtpcCurve, BankRtpcPoint,
    BankRtpcRead, BankRtpcRow, BankSoundStateRead, BankSource, HircLayout, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

use super::bnk_inline_state_fixture::{Fixture, SOUND_ID, sound_bank, source_body, state_bytes};

type TestResult = Result<(), Box<dyn Error>>;

const RTPC_ID: u32 = 0xFA01_8888;
const CURVE_ID: u32 = 0x33EA_88FF;
const NAN_BITS: u32 = 0x7FC1_2345;
const NEGATIVE_ZERO: u32 = 0x8000_0000;
const INTERPOLATION: u32 = 0xF901_0017;

#[test]
fn sound_rtpc_reader_keeps_original_curve_order_and_raw_point_words() -> TestResult {
    let rtpc = curves();
    let fixture = sound_bank(154, &state_bytes(), &rtpc);
    verify_layout(&fixture, HircLayout::Decoded)?;
    with_rtpc(&fixture, |sound| {
        let BankRtpcRead::Readable(mut rows) = sound.status else {
            return Err("checked RTPC region".into());
        };
        let start = sound.object.body_offset() + u64::try_from(fixture.state_in_body.end)?;
        assert_eq!(
            rows.range(),
            &(start..sound.object.body_end_exclusive()),
            "exact region after inline State"
        );
        let count = rows.declared_curve_count();
        assert_eq!(count.value, 2, "original count");
        assert_eq!(
            bytes(&fixture, &count.range)?,
            &2_u16.to_le_bytes(),
            "original count word"
        );
        assert_eq!(rows.checked_curve_count(), 2, "both declared curves fit");
        let rows = rows.by_ref().collect::<Result<Vec<_>, _>>()?;
        let [
            BankRtpcRow::Curve {
                ordinal: 0,
                fields: first,
            },
            BankRtpcRow::Point {
                curve: 0,
                ordinal: 0,
                fields: point,
            },
            BankRtpcRow::Curve {
                ordinal: 1,
                fields: second,
            },
        ] = rows.as_slice()
        else {
            return Err("original curves and points in physical order".into());
        };
        check_curve(&fixture, first, 1)?;
        check_curve(&fixture, second, 0)?;
        check_point(&fixture, point)
    })
}

#[test]
fn truncated_rtpc_keeps_checked_curves_and_earlier_inline_state() -> TestResult {
    let mut rtpc = curves();
    rtpc[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    append_curve(&mut rtpc, u16::MAX);
    let fixture = sound_bank(154, &state_bytes(), &rtpc);
    verify_layout(&fixture, HircLayout::Malformed)?;
    with_rtpc(&fixture, |sound| {
        let BankRtpcRead::Malformed {
            mut rows,
            error: BankError::Truncated { .. },
            ..
        } = sound.status
        else {
            return Err("truncated point extent with checked curve prefix".into());
        };
        assert_eq!(
            rows.declared_curve_count().value,
            u16::MAX,
            "declared curve count retains the encoded bound"
        );
        assert_eq!(
            rows.checked_curve_count(),
            2,
            "only complete curves admitted"
        );
        assert_eq!(
            bytes(&fixture, rows.range())?.len(),
            curves().len(),
            "invalid final curve header stays outside checked prefix"
        );
        assert_eq!(
            rows.by_ref().collect::<Result<Vec<_>, _>>()?.len(),
            3,
            "complete curve and point prefix retained"
        );
        Ok(())
    })?;
    with_source(&fixture, |hirc, header| {
        let sound = hirc.next_sound_states(header).ok_or("original Sound")??;
        let BankSoundStateRead::Readable(rows) = sound.status else {
            return Err("separately checked inline State survives".into());
        };
        assert_eq!(
            rows.collect::<Result<Vec<_>, _>>()?.len(),
            8,
            "all original State observations"
        );
        Ok(())
    })
}

#[test]
fn rtpc_distinguishes_explicit_empty_absent_unsupported_and_unreadable_count() -> TestResult {
    let empty = sound_bank(154, &[0, 0], &[0, 0]);
    with_rtpc(&empty, |sound| {
        let BankRtpcRead::Readable(mut rows) = sound.status else {
            return Err("explicit empty chunk".into());
        };
        assert_eq!(
            rows.declared_curve_count().value,
            0,
            "original empty curve count"
        );
        assert!(rows.next().is_none(), "explicit empty curve stream");
        Ok(())
    })?;
    let absent = Fixture {
        bytes: TestBank::new(77)
            .object(TestHircObject::raw(2, SOUND_ID, source_body()))
            .object(TestHircObject::event(8, &[9]))
            .bytes(),
        state_in_body: 0..0,
    };
    with_rtpc(&absent, |sound| {
        assert!(
            matches!(sound.status, BankRtpcRead::Absent),
            "valid Sound without node"
        );
        Ok(())
    })?;
    let unsupported = sound_bank(153, &[0, 0], &curves());
    with_rtpc(&unsupported, |sound| {
        assert!(
            matches!(sound.status, BankRtpcRead::Unsupported { version: 153 }),
            "older Sound remains opaque"
        );
        Ok(())
    })?;
    let short = sound_bank(154, &[0, 0], &[1]);
    with_rtpc(&short, |sound| {
        assert!(
            matches!(
                sound.status,
                BankRtpcRead::Unavailable(BankError::Truncated { .. })
            ),
            "an unreadable count has no checked rows"
        );
        Ok(())
    })
}

#[test]
fn a_complete_maximum_curve_count_reaches_the_final_original_ordinal() -> TestResult {
    let mut rtpc = u16::MAX.to_le_bytes().to_vec();
    for _ in 0..u16::MAX {
        append_curve(&mut rtpc, 0);
    }
    let fixture = sound_bank(154, &[0, 0], &rtpc);
    with_rtpc(&fixture, |sound| {
        let BankRtpcRead::Readable(rows) = sound.status else {
            return Err("all authored curves fit".into());
        };
        assert_eq!(
            rows.checked_curve_count(),
            u16::MAX,
            "every encoded curve fits"
        );
        let last = rows.last().ok_or("final original curve")??;
        let BankRtpcRow::Curve { ordinal, .. } = last else {
            return Err("zero-point curves emit only headers".into());
        };
        assert_eq!(
            ordinal,
            u16::MAX - 1,
            "last physical ordinal fits the source count"
        );
        Ok(())
    })
}

fn check_point(fixture: &Fixture, point: &BankRtpcPoint) -> TestResult {
    assert_eq!(
        bytes(fixture, &point.from_bits.range)?,
        &NAN_BITS.to_le_bytes(),
        "original input word"
    );
    assert_eq!(
        bytes(fixture, &point.to_bits.range)?,
        &NEGATIVE_ZERO.to_le_bytes(),
        "original output word"
    );
    assert_eq!(
        bytes(fixture, &point.interpolation.range)?,
        &INTERPOLATION.to_le_bytes(),
        "original interpolation word"
    );
    assert_eq!(
        (
            point.from_bits.value,
            point.to_bits.value,
            point.interpolation.value
        ),
        (NAN_BITS, NEGATIVE_ZERO, INTERPOLATION),
        "raw words retain unknown bits"
    );
    assert_eq!(
        (point.input().to_bits(), point.output().to_bits()),
        (NAN_BITS, NEGATIVE_ZERO),
        "IEEE interpretation retains source words"
    );
    Ok(())
}

fn check_curve(fixture: &Fixture, curve: &BankRtpcCurve, points: u16) -> TestResult {
    assert_eq!(
        (
            curve.rtpc_id.value,
            curve.curve_id.value,
            curve.parameter_id.value
        ),
        (RTPC_ID, CURVE_ID, 135),
        "authored IDs including repeated curve and target IDs"
    );
    assert_eq!(
        bytes(fixture, &curve.rtpc_id.range)?,
        &RTPC_ID.to_le_bytes(),
        "original RTPC ID"
    );
    assert_eq!(
        bytes(fixture, &curve.curve_id.range)?,
        &CURVE_ID.to_le_bytes(),
        "original curve ID"
    );
    assert_eq!(
        bytes(fixture, &curve.parameter_id.range)?,
        &[0x81, 0x07],
        "original VLQ"
    );
    for (field, expected) in [
        (&curve.rtpc_type, 0xF1),
        (&curve.accumulation, 0xE2),
        (&curve.scaling, 0xD3),
    ] {
        assert_eq!(
            field.value, expected,
            "unknown original flags remain numeric"
        );
        assert_eq!(
            bytes(fixture, &field.range)?,
            &[expected],
            "original flag range"
        );
    }
    assert_eq!(curve.point_count.value, points, "authored point count");
    assert_eq!(
        bytes(fixture, &curve.point_count.range)?,
        &points.to_le_bytes(),
        "original point count"
    );
    Ok(())
}

fn curves() -> Vec<u8> {
    let mut rtpc = 2_u16.to_le_bytes().to_vec();
    append_curve(&mut rtpc, 1);
    rtpc.extend(
        [NAN_BITS, NEGATIVE_ZERO, INTERPOLATION]
            .into_iter()
            .flat_map(u32::to_le_bytes),
    );
    append_curve(&mut rtpc, 0);
    rtpc
}

fn append_curve(rtpc: &mut Vec<u8>, points: u16) {
    rtpc.extend(RTPC_ID.to_le_bytes());
    rtpc.extend([0xF1, 0xE2, 0x81, 0x07]);
    rtpc.extend(CURVE_ID.to_le_bytes());
    rtpc.push(0xD3);
    rtpc.extend(points.to_le_bytes());
}

fn verify_layout(fixture: &Fixture, expected: HircLayout) -> TestResult {
    let bank = Soundbank::parse(&fixture.bytes)?;
    let sound = bank.objects().first().ok_or("original Sound object")?;
    assert_eq!(
        sound.layout(),
        expected,
        "slice reader independently validates original physical layout"
    );
    Ok(())
}

fn bytes<'source>(
    fixture: &'source Fixture,
    range: &Range<u64>,
) -> Result<&'source [u8], Box<dyn Error>> {
    fixture
        .bytes
        .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
        .ok_or_else(|| "original RTPC range".into())
}

fn with_rtpc(
    fixture: &Fixture,
    check: impl FnOnce(BankHircRtpc<'_, Cursor<&[u8]>>) -> TestResult,
) -> TestResult {
    with_source(fixture, |hirc, header| {
        let sound = hirc.next_sound_rtpc(header).ok_or("physical Sound")??;
        assert_eq!(sound.object.id(), SOUND_ID, "physical Sound identity");
        check(sound)?;
        let event = hirc.next_sound_rtpc(header).ok_or("physical Event")??;
        assert_eq!(
            event.object.id(),
            8,
            "following physical HIRC object remains reachable"
        );
        assert!(
            matches!(event.status, BankRtpcRead::NotSound),
            "Event has no Sound RTPC"
        );
        Ok(())
    })
}

fn with_source<'source>(
    fixture: &'source Fixture,
    check: impl FnOnce(&mut BankHirc<'_, Cursor<&'source [u8]>>, &BankObservedHeader) -> TestResult,
) -> TestResult {
    let mut source = BankSource::open(
        Cursor::new(fixture.bytes.as_slice()),
        u64::try_from(fixture.bytes.len())?,
    )?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, hirc] = sections.as_slice() else {
        return Err("two physical bank sections".into());
    };
    let header = source.read_observed_header_for(header)?;
    let mut hirc = source.hirc(hirc)?;
    check(&mut hirc, &header)
}
