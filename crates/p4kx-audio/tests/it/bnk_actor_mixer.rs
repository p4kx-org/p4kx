// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Cursor;
use std::ops::Range;

use p4kx_audio::{
    BankContainerRead, BankError, BankHircActorMixer, BankHircSemantic, BankSource, HircKind,
    HircLayout, Soundbank,
};
use p4kx_testkit::{TestBank, TestHircObject, TestNodeBase};

use super::bnk_inline_state_fixture::{rtpc_bytes, state_bytes};

type TestResult = Result<(), Box<dyn Error>>;

const TYPE: u8 = 7;
const ID: u32 = 0x6B1A_3299;
const PARENT: u32 = 0x2C21_1B2E;
const IDS: [u32; 3] = [0x9814_5678, 0x9814_5678, 0x3255_9187];
const WORD_LEN: usize = size_of::<u32>();

#[test]
fn actor_mixer_exposes_its_original_parent_and_repeated_ordered_children() -> TestResult {
    let bytes = bank(
        154,
        TestHircObject::actor_mixer(ID, &IDS, TestNodeBase::Full),
    );
    let bank = Soundbank::parse(&bytes)?;
    let actor = bank.objects().first().ok_or("original ActorMixer")?;
    assert_eq!(actor.kind, HircKind::ActorMixer, "original HIRC type 7");
    assert_eq!(actor.id, ID, "original ActorMixer ID");
    let body = bank.object_body(actor);
    let expected: Vec<_> = std::iter::once(3)
        .chain(IDS)
        .flat_map(u32::to_le_bytes)
        .collect();
    assert!(
        body.ends_with(&expected),
        "the original count and repeated IDs are present"
    );
    assert_eq!(
        actor.layout(),
        HircLayout::Decoded,
        "supported complete v154 layout"
    );
    assert_eq!(actor.parent_id(), Some(PARENT), "physical NodeBase parent");
    assert_eq!(
        actor.children().collect::<Vec<_>>(),
        IDS,
        "physical order and duplicates"
    );
    Ok(())
}

#[test]
fn actor_mixer_only_interprets_the_proved_version_and_reports_short_bodies() -> TestResult {
    for version in [153, 155] {
        let bytes = bank(
            version,
            TestHircObject::actor_mixer(ID, &IDS, TestNodeBase::Plain),
        );
        let bank = Soundbank::parse(&bytes)?;
        let object = bank.objects().first().ok_or("physical ActorMixer")?;
        assert_eq!(
            object.layout(),
            HircLayout::Unsupported,
            "version {version}"
        );
        assert!(
            !bank.object_body(object).is_empty(),
            "original unsupported body remains available"
        );
    }
    let full = bank(
        154,
        TestHircObject::actor_mixer(ID, &IDS, TestNodeBase::Plain),
    );
    let original = Soundbank::parse(&full)?;
    let body = original.object_body(original.objects().first().ok_or("source fixture")?);
    let short = body
        .get(WORD_LEN..body.len() - 1)
        .ok_or("truncate the last original child")?;
    let bytes = bank(154, TestHircObject::raw(TYPE, ID, short));
    let bank = Soundbank::parse(&bytes)?;
    let actor = bank
        .objects()
        .first()
        .ok_or("physical truncated ActorMixer")?;
    assert_eq!(
        actor.layout(),
        HircLayout::Malformed,
        "partial child list reports a format error"
    );
    assert!(
        bank.object_body(actor).ends_with(short),
        "malformed original bytes survive"
    );
    Ok(())
}

fn bank(version: u32, actor: TestHircObject) -> Vec<u8> {
    TestBank::new(0xBB)
        .version(version)
        .object(actor)
        .object(TestHircObject::event(0xAC, &[]))
        .bytes()
}

#[test]
fn seekable_actor_mixer_borrows_original_node_chunks_and_child_words() -> TestResult {
    let body = source_body(&rtpc_bytes(), 3, &IDS);
    let bytes = bank(154, TestHircObject::raw(TYPE, ID, body.as_slice()));
    with_actor(&bytes, |actor| {
        assert!(
            matches!(actor.status(), BankContainerRead::Decoded),
            "exact body"
        );
        assert_eq!(actor.object().id(), ID, "physical occurrence");
        let parent = actor
            .fields()
            .node
            .parent_id
            .as_ref()
            .ok_or("parent span")?;
        assert_eq!(parent.value, PARENT, "nonzero shared parent");
        assert_eq!(
            bytes_in(&bytes, &parent.range)?,
            PARENT.to_le_bytes(),
            "the original parent word"
        );
        let count = actor
            .fields()
            .child_count
            .as_ref()
            .ok_or("child count span")?;
        assert_eq!(bytes_in(&bytes, &count.range)?, 3_u32.to_le_bytes());
        let node = actor
            .fields()
            .node_range
            .clone()
            .ok_or("traversed node base")?;
        assert_eq!(
            node,
            actor.object().body_offset() + WORD_LEN as u64..count.range.start
        );
        let checked = actor
            .fields()
            .children
            .clone()
            .ok_or("complete child span")?;
        assert_eq!(
            checked,
            count.range.end..actor.object().body_end_exclusive()
        );
        check_states(&bytes, actor)?;
        let rtpc = actor.rtpc().ok_or("original RTPC region")?;
        assert_eq!(
            rtpc.range().end,
            node.end,
            "RTPC ends before the child count"
        );
        assert_eq!(bytes_in(&bytes, rtpc.range())?, rtpc_bytes());
        assert_eq!(rtpc.declared_curve_count().value, 1);
        assert_eq!(
            rtpc.collect::<Result<Vec<_>, _>>()?.len(),
            2,
            "one curve and one point"
        );
        check_children(&bytes, actor, checked.start, &IDS)
    })
}

#[test]
fn malformed_actor_mixer_preserves_admitted_prefixes_independently() -> TestResult {
    let mut body = source_body(&rtpc_bytes(), u32::MAX, &IDS[..2]);
    body.push(0xAA);
    let bytes = bank(154, TestHircObject::raw(TYPE, ID, body.as_slice()));
    with_actor(&bytes, |actor| {
        assert!(
            matches!(actor.status(), BankContainerRead::Malformed(_)),
            "the complete child extent exceeds the body"
        );
        let count = actor
            .fields()
            .child_count
            .as_ref()
            .ok_or("original impossible count")?;
        assert_eq!(count.value, u32::MAX, "the declared count is retained");
        let start = count.range.end;
        assert_eq!(
            actor.fields().checked_children,
            2,
            "two complete physical words"
        );
        assert!(
            actor.fields().trailing.is_none(),
            "the declared list did not complete"
        );
        check_states(&bytes, actor)?;
        assert_eq!(
            actor
                .rtpc()
                .ok_or("independent complete RTPC")?
                .checked_curve_count(),
            1
        );
        check_children(&bytes, actor, start, &IDS[..2])
    })?;

    let mut rtpc = rtpc_bytes();
    rtpc[..2].copy_from_slice(&2_u16.to_le_bytes());
    rtpc.extend([0x42, 0x13, 0xAC]);
    let mut body = source_body(&rtpc, 0, &[]);
    body.truncate(body.len() - WORD_LEN);
    let bytes = bank(154, TestHircObject::raw(TYPE, ID, body.as_slice()));
    with_actor(&bytes, |actor| {
        assert!(
            matches!(actor.status(), BankContainerRead::Malformed(_)),
            "second RTPC curve is cut off"
        );
        check_states(&bytes, actor)?;
        let rtpc = actor.rtpc().ok_or("checked RTPC curve prefix")?;
        assert_eq!(rtpc.declared_curve_count().value, 2);
        assert_eq!(rtpc.checked_curve_count(), 1);
        assert_eq!(rtpc.collect::<Result<Vec<_>, _>>()?.len(), 2);
        assert!(
            actor.fields().child_count.is_none(),
            "the next count was never reached"
        );
        assert!(actor.children().is_none(), "no manufactured children");
        Ok(())
    })
}

#[test]
fn seekable_actor_mixer_retains_unsupported_and_empty_dispositions() -> TestResult {
    let body = source_body(&rtpc_bytes(), 0, &[]);
    let bytes = bank(153, TestHircObject::raw(TYPE, ID, body.as_slice()));
    with_actor(&bytes, |actor| {
        assert!(
            matches!(
                actor.status(),
                BankContainerRead::Unsupported(BankError::UnsupportedObjectLayout {
                    kind: HircKind::ActorMixer,
                    version: 153,
                })
            ),
            "the raw original kind and unsupported version"
        );
        assert!(
            actor.fields().node_range.is_none(),
            "the unsupported body is never traversed"
        );
        assert!(actor.states().is_none() && actor.rtpc().is_none() && actor.children().is_none());
        Ok(())
    })?;

    let bytes = bank(154, TestHircObject::raw(TYPE, ID, body));
    with_actor(&bytes, |actor| {
        assert!(matches!(actor.status(), BankContainerRead::Decoded));
        let checked = actor
            .fields()
            .children
            .as_ref()
            .ok_or("explicit empty list")?;
        assert_eq!(
            checked,
            &(actor.object().body_end_exclusive()..actor.object().body_end_exclusive())
        );
        assert!(
            actor
                .children()
                .ok_or("cursor for an explicit empty list")?
                .next()
                .is_none()
        );
        Ok(())
    })
}

fn with_actor(
    bytes: &[u8],
    check: impl FnOnce(&mut BankHircActorMixer<'_, Cursor<&[u8]>>) -> TestResult,
) -> TestResult {
    let mut source = BankSource::open(Cursor::new(bytes), u64::try_from(bytes.len())?)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, section] = sections.as_slice() else {
        return Err("two physical sections".into());
    };
    let header = source.read_observed_header_for(header)?;
    let mut hirc = source.hirc(section)?;
    let Some(BankHircSemantic::ActorMixer(mut actor)) =
        hirc.next_semantic_with_node_fields(&header).transpose()?
    else {
        return Err("owner-bound physical ActorMixer".into());
    };
    check(&mut actor)?;
    drop(actor);
    let Some(BankHircSemantic::Event(event)) = hirc.next_semantic(&header).transpose()? else {
        return Err("the adjacent original Event remains readable".into());
    };
    assert_eq!(event.object.id(), 0xAC, "the exact adjacent Event");
    Ok(())
}

fn source_body(rtpc: &[u8], declared_children: u32, ids: &[u32]) -> Vec<u8> {
    let mut body = vec![0, 0, 0, 0];
    body.extend(7_u32.to_le_bytes());
    body.extend(PARENT.to_le_bytes());
    body.extend([0, 0, 0, 0, 0]);
    body.extend(0_u32.to_le_bytes());
    body.extend([0, 1, 0, 0, 3, 0x0C]);
    body.extend(state_bytes());
    body.extend(rtpc);
    body.extend(declared_children.to_le_bytes());
    body.extend(ids.iter().copied().flat_map(u32::to_le_bytes));
    body
}

fn bytes_in<'source>(
    bytes: &'source [u8],
    range: &Range<u64>,
) -> Result<&'source [u8], Box<dyn Error>> {
    bytes
        .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
        .ok_or_else(|| "admitted original bank span".into())
}

fn check_states(bytes: &[u8], actor: &mut BankHircActorMixer<'_, Cursor<&[u8]>>) -> TestResult {
    let state = actor.states().ok_or("independently checked inline State")?;
    assert_eq!(
        bytes_in(bytes, state.range())?,
        state_bytes(),
        "original State bytes"
    );
    assert_eq!(
        state.collect::<Result<Vec<_>, _>>()?.len(),
        8,
        "original State bindings, groups and values"
    );
    Ok(())
}

fn check_children(
    bytes: &[u8],
    actor: &mut BankHircActorMixer<'_, Cursor<&[u8]>>,
    start: u64,
    expected: &[u32],
) -> TestResult {
    let rows = actor
        .children()
        .ok_or("checked child cursor")?
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(rows.len(), expected.len(), "all complete original children");
    for (ordinal, (row, expected)) in rows.iter().zip(expected).enumerate() {
        let position = start + u64::try_from(ordinal * WORD_LEN)?;
        assert_eq!(row.ordinal, u32::try_from(ordinal)?, "authored child order");
        assert_eq!(row.child_id.value, *expected, "the actual child ID");
        assert_eq!(
            row.child_id.range,
            position..position + WORD_LEN as u64,
            "exact child extent"
        );
        assert_eq!(
            bytes_in(bytes, &row.child_id.range)?,
            expected.to_le_bytes(),
            "source child word at the reported range"
        );
    }
    Ok(())
}
