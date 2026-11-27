// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{HircLayout, Soundbank};
use p4kx_media::{MediaRecord, Status};
use p4kx_testkit::{TestContainerKind, TestHircObject, TestNodeBase};
use serde_json::json;

use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

#[path = "audio_bank_container_node_fixture.rs"]
pub(super) mod fixture;

use fixture::{Evidence, bank, export};

const ID: u32 = 0x782A_2283;
const CHILD: u32 = 0x5001;
const PROPERTIES: &str = "audio-bank-hirc-node-properties";
const PROPERTY: &str = "audio-bank-hirc-node-property";
const CONTAINER: &str = "audio-bank-hirc-container";
const EFFECT: &str = "audio-bank-hirc-node-fx-slot";
const AUX: &str = "audio-bank-hirc-node-aux";
const POSITION: &str = "audio-bank-hirc-node-positioning";
const ADVANCED: &str = "audio-bank-hirc-node-advanced-settings";
const STATE: &str = "audio-bank-hirc-inline-state";
const RTPC: &str = "audio-bank-hirc-container-rtpc";

#[test]
fn each_v154_container_replays_the_shared_groups_from_its_own_bank() -> TestResult {
    for (kind, encrypted) in [
        (TestContainerKind::Random, true),
        (TestContainerKind::Sequence, false),
        (TestContainerKind::Switch, true),
        (TestContainerKind::Blend, false),
    ] {
        let bytes = bank(
            154,
            TestHircObject::container(kind, ID, &[CHILD], TestNodeBase::Full),
        );
        export(&bytes, encrypted, HircLayout::Decoded, |rows, evidence| {
            shared(rows, evidence)?;
            layout(
                row(rows, CONTAINER, &[1, 0])?.1,
                "decoded",
                Status::Complete,
            );
            let child = row(rows, "audio-bank-hirc-container-child", &[1, 0, 0])?.1;
            assert_eq!(
                child.data.pointer("/payload/child_id"),
                Some(&json!(CHILD)),
                "the original child cursor is preserved"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn shared(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for namespace in [PROPERTIES, AUX, POSITION, ADVANCED, STATE, RTPC] {
        layout(
            row(rows, namespace, &[1, 0])?.1,
            "decoded",
            Status::Complete,
        );
    }
    evidence.word(
        row(rows, PROPERTY, &[1, 0, 0, 0])?,
        "words/scalar_bits",
        "nodePropertyScalarBits",
        3.0_f32.to_bits(),
    )?;
    evidence.word(
        row(rows, EFFECT, &[1, 0, 0, 0])?,
        "effect_id",
        "nodeFxReferenceId",
        0x3ECB_429B,
    )?;
    evidence.word(
        row(rows, EFFECT, &[1, 0, 1, 0])?,
        "effect_id",
        "nodeFxReferenceId",
        0x0123_4567,
    )?;
    evidence.word(
        row(rows, AUX, &[1, 0])?,
        "observed/user_bus_ids/0",
        "nodeUserAuxBusId",
        0x6300_0001,
    )?;
    evidence.word(
        row(rows, POSITION, &[1, 0])?,
        "observed/transition_time",
        "nodePositioningTransitionTime",
        1000,
    )?;
    evidence.byte(
        row(rows, ADVANCED, &[1, 0])?,
        "observed/virtual_queue_behavior",
        "nodeAdvancedVirtualQueueBehavior",
        1,
    )?;
    shared_state_and_rtpc(rows, evidence)
}

fn shared_state_and_rtpc(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    evidence.word(
        row(
            rows,
            "audio-bank-hirc-inline-state-property",
            &[1, 0, 0, 0, 0],
        )?,
        "value_bits",
        "stateValueBits",
        96.0_f32.to_bits(),
    )?;
    evidence.word(
        row(rows, "audio-bank-hirc-container-rtpc-curve", &[1, 0, 0])?,
        "curve_id",
        "rtpcCurveId",
        0x3E18_0FB3,
    )
}

#[test]
fn a_short_container_tail_retains_complete_shared_groups() -> TestResult {
    let original = bank(
        154,
        TestHircObject::container(TestContainerKind::Random, ID, &[CHILD], TestNodeBase::Full),
    );
    let parsed = Soundbank::parse(&original)?;
    let object = parsed
        .objects()
        .first()
        .ok_or("synthetic Random container")?;
    let body = parsed.object_body(object);
    let clipped = body
        .get(size_of::<u32>()..body.len() - 1)
        .ok_or("one missing container suffix byte")?;
    let bytes = bank(154, TestHircObject::raw(5, ID, clipped));
    export(&bytes, true, HircLayout::Malformed, |rows, evidence| {
        layout(
            row(rows, CONTAINER, &[1, 0])?.1,
            "malformed",
            Status::Partial,
        );
        shared(rows, evidence)
    })
}

#[test]
fn short_node_keeps_early_groups_and_reports_unavailable_later_groups() -> TestResult {
    let original = bank(
        154,
        TestHircObject::container(TestContainerKind::Switch, ID, &[CHILD], TestNodeBase::Full),
    );
    let parsed = Soundbank::parse(&original)?;
    let object = parsed
        .objects()
        .first()
        .ok_or("synthetic Switch container")?;
    let body = parsed.object_body(object);
    let start = size_of::<u32>();
    let property_word_start = start + 17 + 2 * size_of::<u32>() + 1 + 2;
    let prefix = body
        .get(start..property_word_start + size_of::<u16>())
        .ok_or("partial scalar word")?;
    let bytes = bank(154, TestHircObject::raw(6, ID, prefix));
    export(&bytes, true, HircLayout::Malformed, |rows, evidence| {
        layout(
            row(rows, CONTAINER, &[1, 0])?.1,
            "malformed",
            Status::Partial,
        );
        layout(
            row(rows, PROPERTIES, &[1, 0])?.1,
            "malformed",
            Status::Partial,
        );
        let property = row(rows, PROPERTY, &[1, 0, 0, 0])?;
        evidence.byte(property, "property_id", "nodePropertyId", 0)?;
        assert!(
            property
                .1
                .data
                .pointer("/payload/words/scalar_bits")
                .is_none(),
            "a partial word is absent from typed output"
        );
        evidence.word(
            row(rows, EFFECT, &[1, 0, 0, 0])?,
            "effect_id",
            "nodeFxReferenceId",
            0x3ECB_429B,
        )?;
        for namespace in [AUX, POSITION, ADVANCED, STATE, RTPC] {
            layout(
                row(rows, namespace, &[1, 0])?.1,
                "unavailable",
                Status::Complete,
            );
        }
        Ok(())
    })
}

#[test]
fn older_container_layout_stays_raw_with_unsupported_shared_fields() -> TestResult {
    let bytes = bank(
        153,
        TestHircObject::container(TestContainerKind::Blend, ID, &[CHILD], TestNodeBase::Full),
    );
    export(&bytes, false, HircLayout::Unsupported, |rows, _| {
        for namespace in [PROPERTIES, AUX, POSITION, ADVANCED] {
            layout(
                row(rows, namespace, &[1, 0])?.1,
                "unsupported",
                Status::Complete,
            );
        }
        assert!(
            rows.iter()
                .all(|record| record.subresource.namespace != PROPERTY),
            "unsupported node layouts supply no typed values"
        );
        Ok(())
    })
}

fn layout(record: &MediaRecord, expected: &str, status: Status) {
    assert_eq!(
        record.status, status,
        "independent source coverage for {}",
        record.subresource.namespace
    );
    assert_eq!(
        record.data.pointer("/payload/layout"),
        Some(&json!(expected)),
        "original layout for {}",
        record.subresource.namespace
    );
}

#[test]
fn container_child_cursor_remains_compatible_and_source_fields_can_be_replayed_sequentially()
-> TestResult {
    use std::io::Cursor;

    use p4kx_extraction::bank::{BankHircSemantic, BankNodePropertyKind, BankSource};

    let bytes = bank(
        154,
        TestHircObject::container(TestContainerKind::Blend, ID, &[CHILD], TestNodeBase::Full),
    );
    let mut source = BankSource::open(Cursor::new(bytes.as_slice()), u64::try_from(bytes.len())?)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, hirc] = sections.as_slice() else {
        return Err("synthetic BKHD and HIRC sections".into());
    };
    let header = source.read_observed_header_for(header)?;
    {
        let mut objects = source.hirc(hirc)?;
        let Some(BankHircSemantic::Container(container)) =
            objects.next_semantic(&header).transpose()?
        else {
            return Err("original container semantic variant".into());
        };
        let mut children = container.children.ok_or("legacy original child cursor")?;
        assert_eq!(
            children.next().ok_or("original child")??.child_id.value,
            CHILD,
            "legacy cursor retains the same original child"
        );
    }
    let mut objects = source.hirc(hirc)?;
    let Some(BankHircSemantic::ContainerWithNode(mut container)) = objects
        .next_semantic_with_source_fields(&header)
        .transpose()?
    else {
        return Err("source-bound node cursor".into());
    };
    let mut rows = container
        .properties(BankNodePropertyKind::Properties)
        .ok_or("checked original property list")?;
    assert_eq!(
        rows.next().ok_or("original property")??.id.value,
        0,
        "the source cursor reads the original property ID"
    );
    let mut children = container.children().ok_or("source-bound child cursor")?;
    assert_eq!(
        children.next().ok_or("original child")??.child_id.value,
        CHILD,
        "children can still be replayed after node properties"
    );
    Ok(())
}
