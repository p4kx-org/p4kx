// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::audio_bank_state_provider::{row, verify_layout};
use super::support::audio_provider::TestResult;

#[path = "audio_bank_node_fx_fixture.rs"]
mod fixture;

use fixture::{AUX_WORDS, EFFECT_ID, Evidence, NO_FX, NODE_FX, REFLECTIONS, aux, bank, export};

const FX: &str = "audio-bank-hirc-node-fx";
const SLOT: &str = "audio-bank-hirc-node-fx-slot";
const AUX: &str = "audio-bank-hirc-node-aux";

#[test]
fn complete_sound_and_actor_mixer_preserve_physical_fx_slots_and_aux_fields() -> TestResult {
    for (sound, encrypted) in [(true, true), (false, false)] {
        let source = bank(sound, 154, &NODE_FX, &aux(true), true);
        verify_layout(&source, HircLayout::Decoded)?;
        export(&source, encrypted, false, |records, evidence| {
            assert_fx_lists(records, evidence)?;
            assert_fx_slots(records, evidence)?;
            assert_aux(records, evidence)
        })?;
    }
    Ok(())
}

fn assert_fx_lists(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let effects = row(records, FX, &[1, 0, 0])?;
    layout(effects.1, "decoded", Status::Complete);
    evidence.byte(
        effects,
        "observed/override_parent_bits",
        "nodeFxOverrideParentBits",
        0xA1,
    )?;
    evidence.byte(effects, "observed/declared_count", "nodeFxCount", 2)?;
    evidence.byte(
        effects,
        "observed/bypass_all_bits",
        "nodeFxBypassAllBits",
        0xE0,
    )?;
    let metadata = row(records, FX, &[1, 0, 1])?;
    layout(metadata.1, "decoded", Status::Complete);
    evidence.byte(
        metadata,
        "observed/override_parent_bits",
        "nodeMetadataOverrideParentBits",
        0x82,
    )?;
    evidence.byte(metadata, "observed/declared_count", "nodeMetadataCount", 1)?;
    assert!(
        metadata
            .1
            .data
            .pointer("/payload/observed/bypass_all_bits")
            .is_none(),
        "the metadata list has no bypass-all byte"
    );
    Ok(())
}

fn assert_fx_slots(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (list, ordinal, index, reference, bits) in [
        (0, 0, 4, EFFECT_ID, 0x87),
        (0, 1, 4, 0, 0x30),
        (1, 0, 0, EFFECT_ID, 0xFE),
    ] {
        let slot = row(records, SLOT, &[1, 0, list, ordinal])?;
        assert_eq!(slot.1.status, Status::Complete, "all slot fields fit");
        assert_eq!(
            slot.1.data.pointer("/payload/ordinal"),
            Some(&json!(ordinal)),
            "slot rows follow original physical order"
        );
        assert_eq!(
            slot.1.data.pointer("/payload/reference_resolution"),
            Some(&json!("unchecked")),
            "an authored effect ID carries no target resolution"
        );
        evidence.byte(slot, "slot_index", "nodeFxSlotIndex", index)?;
        evidence.word(slot, "effect_id", "nodeFxReferenceId", reference)?;
        evidence.byte(slot, "slot_bits", "nodeFxSlotBits", bits)?;
    }
    Ok(())
}

fn assert_aux(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let routing = row(records, AUX, &[1, 0])?;
    layout(routing.1, "decoded", Status::Complete);
    evidence.byte(routing, "observed/flags", "nodeAuxFlags", 0xE8)?;
    for (ordinal, id) in AUX_WORDS.into_iter().enumerate() {
        evidence.word(
            routing,
            &format!("observed/user_bus_ids/{ordinal}"),
            "nodeUserAuxBusId",
            id,
        )?;
    }
    evidence.word(
        routing,
        "observed/reflections_bus_id",
        "nodeReflectionsAuxBusId",
        REFLECTIONS,
    )?;
    assert_eq!(
        routing
            .1
            .data
            .pointer("/payload/observed/reference_resolution"),
        Some(&json!("unchecked")),
        "authored auxiliary bus IDs carry no target resolution"
    );
    Ok(())
}

#[test]
fn partial_fx_word_keeps_checked_prefix_without_inventing_a_reference() -> TestResult {
    let mut short_effects = NODE_FX.get(..9).ok_or("one original effect slot")?.to_vec();
    short_effects.extend([4, 0xAB, 0xCD]);
    let source = bank(true, 154, &short_effects, &[], false);
    verify_layout(&source, HircLayout::Malformed)?;
    export(&source, true, true, |records, evidence| {
        layout(
            row(records, FX, &[1, 0, 0])?.1,
            "malformed",
            Status::Partial,
        );
        layout(
            row(records, FX, &[1, 0, 1])?.1,
            "unavailable",
            Status::Complete,
        );
        layout(
            row(records, AUX, &[1, 0])?.1,
            "unavailable",
            Status::Complete,
        );
        let complete = row(records, SLOT, &[1, 0, 0, 0])?;
        assert_eq!(
            complete.1.status,
            Status::Complete,
            "the first slot is readable"
        );
        evidence.word(complete, "effect_id", "nodeFxReferenceId", EFFECT_ID)?;
        let partial = row(records, SLOT, &[1, 0, 0, 1])?;
        assert_eq!(
            partial.1.status,
            Status::Partial,
            "the second slot is incomplete"
        );
        evidence.byte(partial, "slot_index", "nodeFxSlotIndex", 4)?;
        for missing in ["effect_id", "slot_bits", "reference_resolution"] {
            assert!(
                partial
                    .1
                    .data
                    .pointer(&format!("/payload/{missing}"))
                    .is_none(),
                "the truncated word supplies no {missing}"
            );
        }
        Ok(())
    })
}

#[test]
fn partial_aux_words_keep_checked_prefix_without_inventing_buses() -> TestResult {
    let mut short_aux = vec![0xE8];
    short_aux.extend(AUX_WORDS.into_iter().take(2).flat_map(u32::to_le_bytes));
    short_aux.extend([0xAB, 0xCD]);
    let source = bank(false, 154, &NO_FX, &short_aux, false);
    verify_layout(&source, HircLayout::Malformed)?;
    export(&source, false, true, |records, evidence| {
        let effects = row(records, FX, &[1, 0, 0])?;
        layout(effects.1, "decoded", Status::Complete);
        evidence.byte(effects, "observed/declared_count", "nodeFxCount", 0)?;
        assert!(
            effects
                .1
                .data
                .pointer("/payload/observed/bypass_all_bits")
                .is_none(),
            "an explicit zero effect count omits the bypass-all byte"
        );
        let routing = row(records, AUX, &[1, 0])?;
        layout(routing.1, "malformed", Status::Partial);
        evidence.word(routing, "observed/user_bus_ids/0", "nodeUserAuxBusId", 0)?;
        evidence.word(
            routing,
            "observed/user_bus_ids/1",
            "nodeUserAuxBusId",
            0x9001_22EF,
        )?;
        assert_eq!(
            routing.1.data.pointer("/payload/observed/user_bus_ids/2"),
            Some(&json!(null)),
            "the next original bus slot is present but its word is incomplete"
        );
        assert!(
            routing
                .1
                .data
                .pointer("/payload/observed/reflections_bus_id")
                .is_none(),
            "the reflections word follows the incomplete send list"
        );
        Ok(())
    })
}

#[test]
fn metadata_with_an_incomplete_flags_byte_preserves_the_whole_effect_reference() -> TestResult {
    let (_, without_tail) = NODE_FX.split_last().ok_or("original metadata slot flags")?;
    let source = bank(false, 154, without_tail, &[], false);
    verify_layout(&source, HircLayout::Malformed)?;
    export(&source, false, true, |records, evidence| {
        layout(row(records, FX, &[1, 0, 0])?.1, "decoded", Status::Complete);
        layout(
            row(records, FX, &[1, 0, 1])?.1,
            "malformed",
            Status::Partial,
        );
        let metadata = row(records, SLOT, &[1, 0, 1, 0])?;
        layout(metadata.1, "malformed", Status::Partial);
        evidence.word(metadata, "effect_id", "nodeFxReferenceId", EFFECT_ID)?;
        assert!(
            metadata.1.data.pointer("/payload/slot_bits").is_none(),
            "the metadata share-set byte was not readable"
        );
        assert_eq!(
            metadata.1.data.pointer("/payload/reference_resolution"),
            Some(&json!("unchecked")),
            "the readable metadata reference remains unchecked"
        );
        Ok(())
    })
}

#[test]
fn omitted_send_ids_and_explicit_zero_reflections_remain_distinct() -> TestResult {
    let source = bank(true, 154, &NO_FX, &aux(false), true);
    verify_layout(&source, HircLayout::Decoded)?;
    export(&source, true, false, |records, evidence| {
        let routing = row(records, AUX, &[1, 0])?;
        layout(routing.1, "decoded", Status::Complete);
        evidence.byte(routing, "observed/flags", "nodeAuxFlags", 0xE0)?;
        evidence.word(
            routing,
            "observed/reflections_bus_id",
            "nodeReflectionsAuxBusId",
            0,
        )?;
        assert!(
            routing
                .1
                .data
                .pointer("/payload/observed/user_bus_ids")
                .is_none(),
            "a cleared user-send flag omits the entire array"
        );
        assert_eq!(
            records
                .iter()
                .filter(|row| row.subresource.namespace == SLOT)
                .count(),
            0,
            "an empty list emits no effect slot rows"
        );
        Ok(())
    })
}

#[test]
fn absent_node_and_unproved_layouts_have_no_authored_fx_or_aux_values() -> TestResult {
    for (source, expected) in [
        (bank(true, 154, &[], &[], false), "absent"),
        (bank(false, 153, &NODE_FX, &aux(true), true), "unsupported"),
        (bank(true, 153, &NODE_FX, &aux(true), true), "unsupported"),
    ] {
        export(&source, false, false, |records, _| {
            for (namespace, indices) in [(FX, &[1, 0, 0][..]), (FX, &[1, 0, 1]), (AUX, &[1, 0])] {
                let record = row(records, namespace, indices)?.1;
                layout(record, expected, Status::Complete);
                assert!(
                    record.data.pointer("/payload/observed").is_none(),
                    "unavailable layout evidence supplies no original values"
                );
            }
            assert_eq!(
                records
                    .iter()
                    .filter(|row| row.subresource.namespace == SLOT)
                    .count(),
                0,
                "no typed slots appear without an observed layout"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn layout(record: &MediaRecord, expected: &str, status: Status) {
    assert_eq!(
        record.status, status,
        "publication status follows the original coverage"
    );
    assert_eq!(
        record.data.pointer("/payload/layout"),
        Some(&json!(expected)),
        "the record distinguishes absence, truncation and unsupported layout"
    );
}
