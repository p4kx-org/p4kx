// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Cursor;
use std::ops::Range;

use p4kx_audio::{
    BankContainerRead, BankHircField, BankHircSemantic, BankHircSoundNode, BankNodePropertyKind,
    BankNodePropertyRows, BankNodePropertyValue, BankSoundContinuation, BankSoundNodeBase,
    BankSoundRead, BankSource, HircLayout, Soundbank,
};

use p4kx_testkit::{TestBank, TestHircObject};

use super::bnk_node_property_fixture::{
    self as fixture, OVERRIDE_BUS_ID, Owner, PROPERTY_BITS, PROPERTY_IDS, RANGED_BITS, RANGED_IDS,
    TestResult,
};

#[test]
fn original_planar_bundles_and_truncations_use_the_existing_v154_node_layout() -> TestResult {
    for owner in [Owner::Sound, Owner::ActorMixer] {
        let full = fixture::complete(owner, 154, OVERRIDE_BUS_ID);
        fixture::assert_layout(&full, owner, HircLayout::Decoded)?;
        let bank = Soundbank::parse(&full)?;
        let object = bank.objects().first().ok_or("original node")?;
        let body = bank.object_body(object);
        let mut expected = fixture::bundle(&PROPERTY_IDS, &PROPERTY_BITS);
        expected.extend(fixture::bundle(&RANGED_IDS, &RANGED_BITS));
        let start = size_of::<u32>() + owner.source_header_len() + 13;
        assert_eq!(
            body.get(start..start + expected.len()),
            Some(expected.as_slice()),
            "exact independent planar property and ranged bytes before positioning"
        );
        fixture::assert_layout(
            &fixture::bank(owner, 154, OVERRIDE_BUS_ID, &[4, 0xF1, 0x03]),
            owner,
            HircLayout::Malformed,
        )?;
        fixture::assert_layout(
            &fixture::bank(owner, 154, 0, &[0, 2, 7, 8, 0, 0, 0, 0, 0xAA]),
            owner,
            HircLayout::Malformed,
        )?;
    }
    Ok(())
}

#[test]
fn node_properties_keep_exact_raw_words_and_authored_bus_and_parent_for_both_owners() -> TestResult
{
    for owner in [Owner::Sound, Owner::ActorMixer] {
        let bytes = fixture::complete(owner, 154, OVERRIDE_BUS_ID);
        with_object(&bytes, |mut object| {
            let parent = match &object {
                BankHircSemantic::SoundWithNode(node) => &sound_base(node)?.fields,
                BankHircSemantic::ActorMixer(node) => &node.fields().node,
                _ => return Err("owner-bound v154 node".into()),
            };
            assert_word(&bytes, parent.override_bus_id(), OVERRIDE_BUS_ID)?;
            assert_word(&bytes, parent.parent_id.as_ref(), fixture::PARENT_ID)?;
            for (kind, ids, words) in [
                (
                    BankNodePropertyKind::Properties,
                    PROPERTY_IDS.as_slice(),
                    PROPERTY_BITS.as_slice(),
                ),
                (
                    BankNodePropertyKind::RangedModifiers,
                    RANGED_IDS.as_slice(),
                    RANGED_BITS.as_slice(),
                ),
            ] {
                let rows = properties(&mut object, kind).ok_or("checked property list")?;
                assert!(
                    rows.extent().is_complete(),
                    "all authored fields are readable"
                );
                assert_rows(&bytes, rows, ids, words)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn truncated_property_ids_keep_the_checked_ids_without_inferred_words() -> TestResult {
    let bytes = fixture::bank(Owner::Sound, 154, OVERRIDE_BUS_ID, &[4, 0xF1, 0x03]);
    with_object(&bytes, |mut object| {
        let BankHircSemantic::SoundWithNode(node) = &object else {
            return Err("actual source-bound Sound".into());
        };
        assert!(
            matches!(node.sound().status, BankSoundRead::Malformed(_)),
            "the incomplete ID array remains malformed"
        );
        assert_word(
            &bytes,
            sound_base(node)?.fields.override_bus_id(),
            OVERRIDE_BUS_ID,
        )?;
        let rows = properties(&mut object, BankNodePropertyKind::Properties)
            .ok_or("actual declared property count")?;
        let extent = rows.extent();
        assert_eq!(
            extent.declared_count.value, 4,
            "original count exceeds readable IDs"
        );
        assert!(
            extent.words_range.is_none(),
            "no location for words until the ID array fits"
        );
        assert!(!extent.is_complete(), "short list remains incomplete");
        assert_rows(&bytes, rows, &[0xF1, 0x03], &[])?;
        assert!(
            properties(&mut object, BankNodePropertyKind::RangedModifiers).is_none(),
            "the later ranged count was never reached"
        );
        Ok(())
    })
}

#[test]
fn malformed_ranged_pair_keeps_the_complete_minimum_and_explicit_empty_properties() -> TestResult {
    let mut lists = vec![0, 2, 0x44, 0xF9];
    let words = [0, 0x7FC1_2345, 0x8000_0000];
    lists.extend(words.into_iter().flat_map(u32::to_le_bytes));
    lists.extend([0xAA, 0xBB]);
    let bytes = fixture::bank(Owner::ActorMixer, 154, 0, &lists);
    with_object(&bytes, |mut object| {
        let BankHircSemantic::ActorMixer(actor) = &object else {
            return Err("actual source-bound ActorMixer".into());
        };
        assert!(
            matches!(actor.status(), BankContainerRead::Malformed(_)),
            "a short ranged word cannot decode the full node"
        );
        assert_word(&bytes, actor.fields().node.override_bus_id(), 0)?;
        let empty = properties(&mut object, BankNodePropertyKind::Properties)
            .ok_or("explicit empty properties")?;
        assert!(
            empty.extent().is_complete(),
            "zero is a complete authored count"
        );
        assert_eq!(
            empty.extent().declared_count.value,
            0,
            "original zero count"
        );
        assert_rows(&bytes, empty, &[], &[])?;
        let ranges = properties(&mut object, BankNodePropertyKind::RangedModifiers)
            .ok_or("checked ranged prefix")?;
        assert!(
            !ranges.extent().is_complete(),
            "the final maximum is cut short"
        );
        assert_rows(&bytes, ranges, &[0x44, 0xF9], &words)
    })
}

#[test]
fn unreadable_count_and_unproved_bank_versions_have_no_property_cursor() -> TestResult {
    let bytes = fixture::bank(Owner::Sound, 154, 0, &[]);
    with_object(&bytes, |mut object| {
        let BankHircSemantic::SoundWithNode(node) = &object else {
            return Err("known v154 Sound layout".into());
        };
        assert!(
            matches!(node.sound().status, BankSoundRead::Malformed(_)),
            "unreadable count remains malformed"
        );
        assert!(
            properties(&mut object, BankNodePropertyKind::Properties).is_none(),
            "count never read"
        );
        Ok(())
    })?;
    let older = fixture::complete(Owner::ActorMixer, 153, OVERRIDE_BUS_ID);
    with_object(&older, |mut object| {
        let BankHircSemantic::ActorMixer(actor) = &object else {
            return Err("original unsupported ActorMixer".into());
        };
        assert!(
            matches!(actor.status(), BankContainerRead::Unsupported(_)),
            "version-qualified raw layout"
        );
        assert!(
            properties(&mut object, BankNodePropertyKind::Properties).is_none(),
            "unproved layout grants no typed extent"
        );
        Ok(())
    })
}

fn sound_base<'a, R>(node: &'a BankHircSoundNode<'_, R>) -> TestResult<&'a BankSoundNodeBase> {
    node.sound()
        .fields
        .node
        .as_ref()
        .and_then(|node| node.base.as_ref())
        .ok_or_else(|| "original node base".into())
}

fn properties<'input, R>(
    object: &'input mut BankHircSemantic<'_, R>,
    kind: BankNodePropertyKind,
) -> Option<BankNodePropertyRows<'input, R>> {
    match object {
        BankHircSemantic::SoundWithNode(node) => node.properties(kind),
        BankHircSemantic::ActorMixer(node) => node.properties(kind),
        _ => None,
    }
}

fn assert_rows(
    bytes: &[u8],
    rows: BankNodePropertyRows<'_, Cursor<&[u8]>>,
    ids: &[u8],
    words: &[u32],
) -> TestResult {
    let extent = rows.extent();
    assert_eq!(
        extent.checked_id_count(),
        u8::try_from(ids.len())?,
        "original readable ID count"
    );
    assert_eq!(
        extent.checked_word_count(),
        u16::try_from(words.len())?,
        "original complete word count"
    );
    assert_eq!(
        span(bytes, &extent.declared_count.range)?,
        &[extent.declared_count.value],
        "count byte at the actual span"
    );
    assert_eq!(span(bytes, &extent.ids_range)?, ids, "original planar IDs");
    if let Some(range) = &extent.words_range {
        let encoded: Vec<_> = words.iter().copied().flat_map(u32::to_le_bytes).collect();
        assert_eq!(span(bytes, range)?, encoded, "original planar words");
    }
    let read = rows.collect::<Result<Vec<_>, _>>()?;
    assert_eq!(read.len(), ids.len(), "one row per checked ID");
    let mut expected_words = words.iter().copied();
    for (ordinal, (row, &id)) in read.iter().zip(ids).enumerate() {
        assert_eq!(row.ordinal, u8::try_from(ordinal)?, "physical ID position");
        assert_eq!(row.id.value, id, "raw ID including unknowns and duplicates");
        assert_eq!(span(bytes, &row.id.range)?, &[id], "exact ID source span");
        match &row.value {
            BankNodePropertyValue::Scalar(word) => {
                assert_optional_word(bytes, word.as_ref(), expected_words.next())?;
            }
            BankNodePropertyValue::Ranged {
                minimum, maximum, ..
            } => {
                assert_optional_word(bytes, minimum.as_ref(), expected_words.next())?;
                assert_optional_word(bytes, maximum.as_ref(), expected_words.next())?;
            }
            _ => return Err("unexpected property word layout".into()),
        }
    }
    assert!(
        expected_words.next().is_none(),
        "every checked word retained"
    );
    Ok(())
}

fn assert_optional_word(
    bytes: &[u8],
    word: Option<&BankHircField<u32>>,
    expected: Option<u32>,
) -> TestResult {
    let Some(expected) = expected else {
        assert!(
            word.is_none(),
            "no value is inferred beyond the original extent"
        );
        return Ok(());
    };
    assert_word(bytes, word, expected)
}

fn assert_word(bytes: &[u8], word: Option<&BankHircField<u32>>, expected: u32) -> TestResult {
    let word = word.ok_or("original readable word")?;
    assert_eq!(
        word.value, expected,
        "raw bits without floating-point conversion"
    );
    assert_eq!(
        span(bytes, &word.range)?,
        expected.to_le_bytes(),
        "exact original word extent"
    );
    Ok(())
}

fn span<'a>(bytes: &'a [u8], range: &Range<u64>) -> TestResult<&'a [u8]> {
    bytes
        .get(usize::try_from(range.start)?..usize::try_from(range.end)?)
        .ok_or_else(|| "original checked byte span".into())
}

fn with_object(
    bytes: &[u8],
    check: impl FnOnce(BankHircSemantic<'_, Cursor<&[u8]>>) -> TestResult,
) -> TestResult {
    let mut source = BankSource::open(Cursor::new(bytes), u64::try_from(bytes.len())?)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, hirc] = sections.as_slice() else {
        return Err("original BKHD and HIRC sections".into());
    };
    let header = source.read_observed_header_for(header)?;
    let mut objects = source.hirc(hirc)?;
    check(
        objects
            .next_semantic_with_node_fields(&header)
            .ok_or("original source object")??,
    )?;
    let Some(BankHircSemantic::Event(next)) = objects.next_semantic(&header).transpose()? else {
        return Err("adjacent physical Event survives the checked property prefix".into());
    };
    assert_eq!(next.object.id(), 1, "original adjacent Event");
    Ok(())
}

#[test]
fn scalar_word_prefix_keeps_all_readable_ids_after_the_last_complete_word() -> TestResult {
    let ids = [5, 0xD1, 0xD1];
    let words = [0];
    let mut list = fixture::bundle(&ids, &words);
    list.extend([0xAA, 0xBB]);
    let bytes = fixture::bank(Owner::Sound, 154, 0, &list);
    with_object(&bytes, |mut object| {
        let rows = properties(&mut object, BankNodePropertyKind::Properties)
            .ok_or("original declared scalar list")?;
        assert!(
            !rows.extent().is_complete(),
            "the final scalar bytes are partial"
        );
        assert_rows(&bytes, rows, &ids, &words)
    })
}

#[test]
fn a_sound_with_no_node_has_no_authored_property_count() -> TestResult {
    let bytes = fixture::complete(Owner::Sound, 154, 0);
    let bank = Soundbank::parse(&bytes)?;
    let object = bank.objects().first().ok_or("original Sound")?;
    let id_len = size_of::<u32>();
    let source = bank
        .object_body(object)
        .get(id_len..id_len + Owner::Sound.source_header_len())
        .ok_or("exact source header before the optional node")?;
    let bytes = TestBank::new(4)
        .object(TestHircObject::raw(2, fixture::OBJECT_ID, source))
        .object(TestHircObject::event(1, &[]))
        .bytes();
    with_object(&bytes, |mut object| {
        let BankHircSemantic::SoundWithNode(node) = &object else {
            return Err("original version-154 Sound".into());
        };
        assert!(
            matches!(
                node.sound().fields.continuation,
                BankSoundContinuation::Absent
            ),
            "the authored body ends before the optional node"
        );
        assert!(
            properties(&mut object, BankNodePropertyKind::Properties).is_none(),
            "a missing node cannot provide an explicit zero count"
        );
        Ok(())
    })
}
