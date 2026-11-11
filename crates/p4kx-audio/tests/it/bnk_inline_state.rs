// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;
use std::ops::Range;

use p4kx_audio::{
    BankError, BankHircSemantic, BankHircSoundStates, BankInlineStateRow, BankSoundRead,
    BankSoundStateRead, BankSource, HircLayout, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject};

#[path = "bnk_inline_state_fixture.rs"]
mod fixture;

use self::fixture::{
    BINDING_ID, Fixture, GROUP_ID, PARENT_ID, PROPERTY_IDS, SOUND_ID, STATE_ID, VALUE_BITS,
    rtpc_bytes, sound_bank, source_body, state_bytes,
};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn authored_inline_states_fit_the_original_sound_between_node_prefix_and_rtpc() -> TestResult {
    let states = state_bytes();
    let fixture = sound_bank(154, &states, &rtpc_bytes());
    let bank = Soundbank::parse(fixture.bytes.as_slice())?;
    let sound = bank.objects().first().ok_or("physical Sound")?;
    assert_eq!(sound.id, SOUND_ID, "original Sound identity");
    assert_eq!(
        sound.parent_id(),
        Some(PARENT_ID),
        "existing parent accessor"
    );
    assert_eq!(
        sound.layout(),
        HircLayout::Decoded,
        "entire original node is valid"
    );
    assert_eq!(
        bank.object_body(sound).get(fixture.state_in_body.clone()),
        Some(states.as_slice()),
        "source State bytes between the exact node prefix and the original RTPC"
    );
    let mut source = BankSource::open(
        Cursor::new(fixture.bytes.as_slice()),
        u64::try_from(fixture.bytes.len())?,
    )?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, section] = sections.as_slice() else {
        return Err("two physical sections".into());
    };
    let header = source.read_observed_header_for(header)?;
    let mut hirc = source.hirc(section)?;
    let Some(BankHircSemantic::Sound(sound)) = hirc.next_semantic(&header).transpose()? else {
        return Err("physical source-bound Sound".into());
    };
    let BankSoundRead::Decoded(decoded) = sound.status else {
        return Err("decoded original Sound".into());
    };
    assert_eq!(
        decoded.parent_id,
        Some(PARENT_ID),
        "source-bound reader preserves the parent"
    );
    let node = sound
        .fields
        .node
        .and_then(|node| node.base)
        .ok_or("read original node")?;
    assert_eq!(
        node.consumed.end,
        sound.object.body_end_exclusive(),
        "existing reader consumes the independent RTPC fixture"
    );
    Ok(())
}

#[test]
fn inline_state_rows_preserve_repeated_ids_raw_values_and_physical_spans() -> TestResult {
    let fixture = sound_bank(154, &state_bytes(), &rtpc_bytes());
    with_sound_states(&fixture, |sound| {
        assert_eq!(sound.object.id(), SOUND_ID, "exact physical Sound");
        let BankSoundStateRead::Readable(mut reader) = sound.status else {
            return Err("observed inline State region".into());
        };
        let body = sound.object.body_offset();
        let expected = body + u64::try_from(fixture.state_in_body.start)?
            ..body + u64::try_from(fixture.state_in_body.end)?;
        assert_eq!(
            reader.range(),
            &expected,
            "cursor bound excludes original RTPC bytes"
        );
        let rows = reader.by_ref().collect::<Result<Vec<_>, _>>()?;
        let [
            count,
            binding_one,
            binding_two,
            group_count,
            group_one,
            state_one,
            state_two,
            group_two,
        ] = rows.as_slice()
        else {
            return Err("all eight original State chunk observations".into());
        };
        check_bindings(&fixture.bytes, count, binding_one, binding_two)?;
        let BankInlineStateRow::GroupCount(groups) = group_count else {
            return Err("group count".into());
        };
        assert_eq!(groups.value, 2, "authored group count");
        assert_eq!(
            bytes_in(&fixture.bytes, &groups.range)?,
            &[2],
            "actual group count word"
        );
        check_group(&fixture.bytes, group_one, 0, 0xE1, 2)?;
        check_group(&fixture.bytes, group_two, 1, 0, 0)?;
        check_states(&fixture.bytes, state_one, state_two)
    })
}

#[test]
fn a_complete_state_chunk_survives_a_malformed_following_rtpc() -> TestResult {
    let fixture = sound_bank(154, &state_bytes(), &[1]);
    let bank = Soundbank::parse(fixture.bytes.as_slice())?;
    let sound = bank.objects().first().ok_or("original malformed Sound")?;
    assert_eq!(
        sound.layout(),
        HircLayout::Malformed,
        "actual short RTPC invalidates Sound layout"
    );
    with_sound_states(&fixture, |sound| {
        let BankSoundStateRead::Readable(reader) = sound.status else {
            return Err("independently verified State region".into());
        };
        assert_eq!(
            reader.collect::<Result<Vec<_>, _>>()?.len(),
            8,
            "complete earlier State region remains readable"
        );
        Ok(())
    })
}

#[test]
fn malformed_and_unsupported_state_extents_remain_distinct() -> TestResult {
    let malformed = sound_bank(154, &[0xff; 6], &[]);
    with_sound_states(&malformed, |sound| {
        assert!(
            matches!(
                sound.status,
                BankSoundStateRead::Unavailable(BankError::InvalidSection { .. })
            ),
            "overlong source VLQ rejects State extent"
        );
        Ok(())
    })?;
    let unsupported = sound_bank(153, &state_bytes(), &rtpc_bytes());
    with_sound_states(&unsupported, |sound| {
        assert!(
            matches!(
                sound.status,
                BankSoundStateRead::Unsupported { version: 153 }
            ),
            "previous Sound version remains opaque"
        );
        Ok(())
    })
}

#[test]
fn empty_inline_state_chunk_differs_from_an_absent_node_base() -> TestResult {
    let empty = sound_bank(154, &[0, 0], &[0, 0]);
    with_sound_states(&empty, |sound| {
        let BankSoundStateRead::Readable(reader) = sound.status else {
            return Err("explicit empty State extent".into());
        };
        let rows = reader.collect::<Result<Vec<_>, _>>()?;
        let [
            BankInlineStateRow::BindingCount(bindings),
            BankInlineStateRow::GroupCount(groups),
        ] = rows.as_slice()
        else {
            return Err("original empty count words".into());
        };
        assert_eq!(
            (bindings.value, groups.value),
            (0, 0),
            "original explicit zero counts"
        );
        Ok(())
    })?;
    let no_node = Fixture {
        bytes: TestBank::new(77)
            .version(154)
            .object(TestHircObject::raw(2, SOUND_ID, source_body()))
            .object(TestHircObject::event(8, &[9]))
            .bytes(),
        state_in_body: 0..0,
    };
    with_sound_states(&no_node, |sound| {
        assert!(
            matches!(sound.status, BankSoundStateRead::Absent),
            "Sound without node has no State chunk"
        );
        Ok(())
    })
}

fn check_bindings(
    source: &[u8],
    count: &BankInlineStateRow,
    first: &BankInlineStateRow,
    second: &BankInlineStateRow,
) -> TestResult {
    let BankInlineStateRow::BindingCount(count) = count else {
        return Err("binding count".into());
    };
    assert_eq!(count.value, 2, "authored binding count");
    assert_eq!(
        bytes_in(source, &count.range)?,
        &[2],
        "actual binding count word"
    );
    for (row, (position, accumulation, in_db)) in [(first, (0, 0xF0, 0x81)), (second, (1, 0, 1))] {
        let BankInlineStateRow::Binding { ordinal, fields } = row else {
            return Err("binding row".into());
        };
        assert_eq!(
            (
                *ordinal,
                fields.property_id.value,
                fields.accumulation.value,
                fields.in_db.value
            ),
            (position, BINDING_ID, accumulation, in_db),
            "repeated property ID and original unknown flag bits"
        );
        assert_eq!(
            bytes_in(source, &fields.property_id.range)?,
            &[0x82, 0x02],
            "actual multi-byte property ID"
        );
        assert_eq!(
            bytes_in(source, &fields.accumulation.range)?,
            &[accumulation],
            "actual accumulation byte"
        );
        assert_eq!(
            bytes_in(source, &fields.in_db.range)?,
            &[in_db],
            "actual in-db byte"
        );
    }
    Ok(())
}

fn check_group(
    source: &[u8],
    row: &BankInlineStateRow,
    position: u32,
    sync: u8,
    count: u32,
) -> TestResult {
    let BankInlineStateRow::Group { ordinal, fields } = row else {
        return Err("original group".into());
    };
    assert_eq!(
        (
            *ordinal,
            fields.group_id.value,
            fields.synchronization.value,
            fields.state_count.value
        ),
        (position, GROUP_ID, sync, count),
        "repeated group ID remains physically separate"
    );
    assert_eq!(
        bytes_in(source, &fields.group_id.range)?,
        &GROUP_ID.to_le_bytes(),
        "actual group ID word"
    );
    assert_eq!(
        bytes_in(source, &fields.synchronization.range)?,
        &[sync],
        "actual synchronization byte"
    );
    Ok(())
}

fn check_states(
    source: &[u8],
    first: &BankInlineStateRow,
    second: &BankInlineStateRow,
) -> TestResult {
    let BankInlineStateRow::State {
        group: 0,
        ordinal: 0,
        state_id,
        properties,
    } = first
    else {
        return Err("first original State".into());
    };
    let BankInlineStateRow::State {
        group: 0,
        ordinal: 1,
        state_id: other_id,
        properties: empty,
    } = second
    else {
        return Err("second original State".into());
    };
    assert_eq!(
        (state_id.value, other_id.value),
        (STATE_ID, STATE_ID),
        "repeated State IDs remain separate"
    );
    assert_eq!(
        bytes_in(source, &state_id.range)?,
        &STATE_ID.to_le_bytes(),
        "actual original State ID"
    );
    assert_eq!(empty.count().value, 0, "second State explicitly empty");
    for (property, (id, bits)) in properties
        .entries()
        .zip(PROPERTY_IDS.into_iter().zip(VALUE_BITS))
    {
        assert_eq!(
            (property.id.value, property.value_bits.value),
            (id, bits),
            "unknown ID and exact nonfinite/signed-zero bits"
        );
        assert_eq!(
            bytes_in(source, &property.id.range)?,
            &id.to_le_bytes(),
            "actual original ID"
        );
        assert_eq!(
            bytes_in(source, &property.value_bits.range)?,
            &bits.to_le_bytes(),
            "actual original value"
        );
    }
    Ok(())
}

fn with_sound_states(
    fixture: &Fixture,
    check: impl FnOnce(BankHircSoundStates<'_, Cursor<&[u8]>>) -> TestResult,
) -> TestResult {
    let mut source = BankSource::open(
        Cursor::new(fixture.bytes.as_slice()),
        u64::try_from(fixture.bytes.len())?,
    )?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, hirc] = sections.as_slice() else {
        return Err("two physical sections".into());
    };
    let header = source.read_observed_header_for(header)?;
    let mut hirc = source.hirc(hirc)?;
    check(hirc.next_sound_states(&header).ok_or("physical Sound")??)?;
    let next = hirc
        .next_sound_states(&header)
        .ok_or("following physical Event")??;
    assert_eq!(
        next.object.id(),
        8,
        "cursor remains on next physical object"
    );
    assert!(
        matches!(next.status, BankSoundStateRead::NotSound),
        "Event has no Sound State rows"
    );
    Ok(())
}

fn bytes_in<'source>(
    source: &'source [u8],
    range: &Range<u64>,
) -> Result<&'source [u8], Box<dyn Error>> {
    source
        .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
        .ok_or_else(|| "original State field range".into())
}
