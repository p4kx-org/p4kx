// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

#[path = "audio_bank_action_properties_fixture.rs"]
mod fixture;

use fixture::{
    BANK_TYPE, Evidence, FADE_FLAGS, PLAY, PROPERTY_BITS, PROPERTY_IDS, RANGE_BITS, RANGE_IDS,
    STOP, TARGET, TARGET_FLAGS, bank, bundle, complete, export,
};

const SUMMARY: &str = "audio-bank-hirc-action-properties";
const PROPERTY: &str = "audio-bank-hirc-action-property";
const ACTION: &str = "audio-bank-hirc-action";
const TARGET_ROW: &str = "audio-bank-hirc-action-target-resolution";

#[test]
fn v154_play_keeps_original_property_words_flags_and_legacy_target_resolution() -> TestResult {
    let source = bank(154, PLAY, &complete()?);
    export(&source, true, HircLayout::Decoded, |records, evidence| {
        let summary = row(records, SUMMARY, &[1, 0])?;
        layout(summary.1, "decoded", Status::Complete);
        evidence.byte(
            summary,
            "observed/target_flags",
            "actionTargetFlags",
            TARGET_FLAGS,
        )?;
        evidence.byte(
            summary,
            "observed/fade_flags",
            "actionFadeFlags",
            FADE_FLAGS,
        )?;
        evidence.byte(
            summary,
            "observed/properties/declared_count",
            "actionPropertyCount",
            3,
        )?;
        evidence.byte(
            summary,
            "observed/ranged_modifiers/declared_count",
            "actionPropertyCount",
            2,
        )?;
        check_words(records, evidence)?;
        let action = row(records, ACTION, &[1, 0])?.1;
        assert_eq!(
            action.data.pointer("/payload/play_bank/bank_id"),
            Some(&json!(0)),
            "the explicit local bank decision remains unchanged"
        );
        assert_eq!(
            action.data.pointer("/payload/play_bank/bank_type"),
            Some(&json!(BANK_TYPE)),
            "the original bank type remains intact"
        );
        let reference = row(records, TARGET_ROW, &[1, 0])?.1;
        assert_eq!(
            reference.data.pointer("/payload/target_id"),
            Some(&json!(TARGET)),
            "legacy target resolution still sees the same target"
        );
        assert_eq!(
            reference.data.pointer("/payload/local/matching_candidates"),
            Some(&json!("1")),
            "the same physical bank-local target matches"
        );
        Ok(())
    })
}

fn check_words(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (ordinal, (&id, &bits)) in PROPERTY_IDS.iter().zip(&PROPERTY_BITS).enumerate() {
        let row = row(records, PROPERTY, &[1, 0, 0, u64::try_from(ordinal)?])?;
        evidence.byte(row, "property_id", "actionPropertyId", id)?;
        evidence.word(row, "words/scalar_bits", "actionPropertyScalarBits", bits)?;
    }
    for (ordinal, (&id, [min, max])) in RANGE_IDS
        .iter()
        .zip(RANGE_BITS.as_chunks::<2>().0)
        .enumerate()
    {
        let row = row(records, PROPERTY, &[1, 0, 1, u64::try_from(ordinal)?])?;
        evidence.byte(row, "property_id", "actionPropertyId", id)?;
        evidence.word(row, "words/minimum_bits", "actionPropertyMinimumBits", *min)?;
        evidence.word(row, "words/maximum_bits", "actionPropertyMaximumBits", *max)?;
    }
    Ok(())
}

#[test]
fn short_ranged_values_keep_complete_minimum_and_explicit_empty_properties() -> TestResult {
    let mut tail = vec![TARGET_FLAGS, 0];
    tail.extend(bundle(&RANGE_IDS, &RANGE_BITS[..3])?);
    tail.extend([0xAE, 0xB0]);
    let source = bank(154, PLAY, &tail);
    export(
        &source,
        false,
        HircLayout::Malformed,
        |records, evidence| {
            let summary = row(records, SUMMARY, &[1, 0])?;
            layout(summary.1, "malformed", Status::Partial);
            evidence.byte(
                summary,
                "observed/properties/declared_count",
                "actionPropertyCount",
                0,
            )?;
            assert_eq!(
                summary
                    .1
                    .data
                    .pointer("/payload/observed/properties/complete"),
                Some(&json!(true)),
                "zero is an authored empty list"
            );
            assert!(
                summary
                    .1
                    .data
                    .pointer("/payload/observed/fade_flags")
                    .is_none(),
                "fade flags follow the incomplete property words"
            );
            let partial = row(records, PROPERTY, &[1, 0, 1, 1])?;
            evidence.byte(partial, "property_id", "actionPropertyId", RANGE_IDS[1])?;
            evidence.word(
                partial,
                "words/minimum_bits",
                "actionPropertyMinimumBits",
                0x8000_0000,
            )?;
            assert!(
                partial
                    .1
                    .data
                    .pointer("/payload/words/maximum_bits")
                    .is_none(),
                "a partial maximum word remains only in the raw bytes"
            );
            assert!(
                records
                    .iter()
                    .all(|record| record.subresource.namespace != TARGET_ROW),
                "a malformed Play does not invent a resolved target"
            );
            Ok(())
        },
    )
}

#[test]
fn short_id_plane_preserves_readable_ids_without_inventing_word_positions() -> TestResult {
    let source = bank(154, PLAY, &[TARGET_FLAGS, 4, 0xA0, 0x03]);
    export(&source, true, HircLayout::Malformed, |records, evidence| {
        let summary = row(records, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.byte(
            summary,
            "observed/properties/declared_count",
            "actionPropertyCount",
            4,
        )?;
        for (ordinal, id) in [0xA0, 0x03].into_iter().enumerate() {
            let row = row(records, PROPERTY, &[1, 0, 0, u64::try_from(ordinal)?])?;
            evidence.byte(row, "property_id", "actionPropertyId", id)?;
            assert!(
                row.1.data.pointer("/payload/words/scalar_bits").is_none(),
                "the value plane has no proven start"
            );
        }
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/ranged_modifiers")
                .is_none(),
            "the later list count is unreachable"
        );
        Ok(())
    })
}

#[test]
fn completed_properties_remain_covered_when_the_bank_reference_tail_is_short() -> TestResult {
    let full = complete()?;
    let tail = full
        .get(..full.len() - size_of::<u32>())
        .ok_or("Play body without bank type")?;
    let source = bank(154, PLAY, tail);
    export(&source, true, HircLayout::Malformed, |records, evidence| {
        layout(
            row(records, ACTION, &[1, 0])?.1,
            "malformed",
            Status::Partial,
        );
        let summary = row(records, SUMMARY, &[1, 0])?;
        layout(summary.1, "decoded", Status::Complete);
        evidence.byte(
            summary,
            "observed/fade_flags",
            "actionFadeFlags",
            FADE_FLAGS,
        )?;
        check_words(records, evidence)
    })
}

#[test]
fn non_play_and_other_versions_keep_existing_action_layout_without_property_rows() -> TestResult {
    for (version, code, expected) in [(153, PLAY, "unsupported"), (154, STOP, "absent")] {
        let source = bank(version, code, &complete()?);
        export(&source, false, HircLayout::Decoded, |records, _| {
            layout(
                row(records, ACTION, &[1, 0])?.1,
                "decoded",
                Status::Complete,
            );
            let summary = row(records, SUMMARY, &[1, 0])?;
            layout(summary.1, expected, Status::Complete);
            assert!(
                summary.1.data.pointer("/payload/observed").is_none(),
                "the reader has no original list proof for {expected}"
            );
            assert!(
                records
                    .iter()
                    .all(|record| record.subresource.namespace != PROPERTY),
                "no typed property rows appear in an unsupported Action branch"
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn only_the_opt_in_cursor_borrows_play_property_rows_from_the_bank() -> TestResult {
    use std::io::Cursor;

    use p4kx_extraction::bank::{BankHircSemantic, BankNodePropertyKind, BankSource};

    let bytes = bank(154, PLAY, &complete()?);
    let mut source = BankSource::open(Cursor::new(bytes.as_slice()), u64::try_from(bytes.len())?)?;
    let sections = source.sections().collect::<Result<Vec<_>, _>>()?;
    let [header, hirc] = sections.as_slice() else {
        return Err("synthetic BKHD and HIRC sections".into());
    };
    let header = source.read_observed_header_for(header)?;
    assert!(
        matches!(
            source.hirc(hirc)?.next_semantic(&header).transpose()?,
            Some(BankHircSemantic::Action(_))
        ),
        "ordinary semantic iteration retains the established Action variant",
    );
    assert!(
        matches!(
            source
                .hirc(hirc)?
                .next_semantic_with_node_fields(&header)
                .transpose()?,
            Some(BankHircSemantic::Action(_))
        ),
        "the existing Sound node cursor retains the established Action variant",
    );
    let mut objects = source.hirc(hirc)?;
    let Some(BankHircSemantic::ActionWithProperties(mut action)) = objects
        .next_semantic_with_source_fields(&header)
        .transpose()?
    else {
        return Err("source fields opt in to the original Play property cursor".into());
    };
    let rows = action
        .properties(BankNodePropertyKind::Properties)
        .ok_or("ordinary property extent")?;
    let ids = rows
        .map(|row| row.map(|row| row.id.value))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        ids, PROPERTY_IDS,
        "the bound cursor reads the same checked original IDs"
    );
    Ok(())
}

#[test]
fn unreached_play_branch_keeps_action_failure_and_explicitly_unavailable_properties() -> TestResult
{
    let mut body = PLAY.to_le_bytes().to_vec();
    body.extend([0xCD, 0xAB]);
    let source = fixture::original_bank(154, body);
    export(&source, false, HircLayout::Malformed, |records, _| {
        layout(
            row(records, ACTION, &[1, 0])?.1,
            "malformed",
            Status::Partial,
        );
        let summary = row(records, SUMMARY, &[1, 0])?;
        layout(summary.1, "unavailable", Status::Complete);
        assert!(
            summary.1.data.pointer("/payload/observed").is_none(),
            "the original target word did not finish"
        );
        assert!(
            records
                .iter()
                .all(|record| record.subresource.namespace != PROPERTY),
            "no property words are reachable before the target ID"
        );
        Ok(())
    })
}

fn layout(record: &MediaRecord, expected: &str, status: Status) {
    assert_eq!(record.status, status, "Action publication coverage");
    assert_eq!(
        record.data.pointer("/payload/layout"),
        Some(&json!(expected)),
        "Action property layout"
    );
}
