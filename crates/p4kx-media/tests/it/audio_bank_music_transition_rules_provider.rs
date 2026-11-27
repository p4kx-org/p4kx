// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::Status;
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{export, layout};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

#[path = "audio_bank_music_transition_rule_evidence.rs"]
mod evidence;
#[path = "audio_bank_music_transition_rules_fixture.rs"]
mod fixture;

use self::evidence::{object_words, reference_order, rule_words, signed};

use self::fixture::{
    DESTINATION_IDS, OBJECT_ID, Owner, SOURCE_IDS, rule, source, transition_object, words,
};

const SUMMARY: &str = "audio-bank-music-transition-rules";
const RULE: &str = "audio-bank-music-transition-rule";
const ID: &str = "audio-bank-music-transition-id";

#[test]
fn version_154_transition_rules_preserve_original_controls_and_order_for_both_owners() -> TestResult
{
    let mut authored = 2_u32.to_le_bytes().to_vec();
    rule(&mut authored, &SOURCE_IDS, &DESTINATION_IDS, 0)?;
    rule(&mut authored, &[], &DESTINATION_IDS[..1], 0xA0)?;
    transition_object(&mut authored);
    for (owner, encrypted) in [(Owner::Switch, true), (Owner::Playlist, false)] {
        let bank = source(owner, 154, &authored, true)?;
        export(&bank, encrypted, HircLayout::Decoded, |rows, evidence| {
            let summary = row(rows, SUMMARY, &[1, 0])?;
            layout(summary.1, "decoded", Status::Complete);
            evidence.word(
                summary,
                "observed/declared_count",
                "musicTransitionRuleCount",
                2,
            )?;
            assert_eq!(
                summary.1.data.pointer("/payload/observed/complete_rules"),
                Some(&json!(2)),
                "both authored rules fit their original body"
            );
            let first = row(rows, RULE, &[1, 0, 0])?;
            layout(first.1, "decoded", Status::Complete);
            rule_words(first, evidence)?;
            reference_order(rows, evidence)?;
            object_words(rows, evidence)
        })?;
    }
    Ok(())
}

#[test]
fn huge_authored_counts_retain_complete_rule_and_partial_next_id_list() -> TestResult {
    let mut authored = u32::MAX.to_le_bytes().to_vec();
    rule(&mut authored, &[], &[], 0)?;
    words(&mut authored, [u32::MAX, 0xAC12_0010]);
    authored.extend([0xDD, 0xE1]);
    let bank = source(Owner::Playlist, 154, &authored, false)?;
    export(&bank, true, HircLayout::Malformed, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.word(
            summary,
            "observed/declared_count",
            "musicTransitionRuleCount",
            u32::MAX,
        )?;
        assert_eq!(
            summary.1.data.pointer("/payload/observed/complete_rules"),
            Some(&json!(1)),
            "the first complete rule is kept independently of the impossible count"
        );
        layout(row(rows, RULE, &[1, 0, 0])?.1, "decoded", Status::Complete);
        let partial = row(rows, RULE, &[1, 0, 1])?;
        layout(partial.1, "malformed", Status::Partial);
        evidence.word(
            partial,
            "source_ids/declared_count",
            "musicTransitionSourceIdCount",
            u32::MAX,
        )?;
        assert!(
            partial.1.data.pointer("/payload/destination_ids").is_none(),
            "the destination list is beyond the failed source list"
        );
        let id = row(rows, ID, &[1, 0, 1, 0, 0])?;
        evidence.word(id, "id", "musicTransitionReferenceId", 0xAC12_0010)?;
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == ID)
                .count(),
            1,
            "a partial ID is never fabricated"
        );
        Ok(())
    })
}

#[test]
fn truncated_allocated_object_retains_its_readable_fade_prefix() -> TestResult {
    let mut authored = 1_u32.to_le_bytes().to_vec();
    rule(&mut authored, &[], &[], 0xE8)?;
    words(
        &mut authored,
        [
            OBJECT_ID,
            u32::from_le_bytes((-21_i32).to_le_bytes()),
            0xFA00_3300,
        ],
    );
    authored.extend([0xCA, 0xDB]);
    let bank = source(Owner::Switch, 154, &authored, false)?;
    export(&bank, false, HircLayout::Malformed, |rows, evidence| {
        let partial = row(rows, RULE, &[1, 0, 0])?;
        layout(partial.1, "malformed", Status::Partial);
        evidence.byte(
            partial,
            "allocation_flag",
            "musicTransitionAllocationFlag",
            0xE8,
        )?;
        evidence.word(
            partial,
            "transition_object/segment_id",
            "musicTransitionObjectSegmentId",
            OBJECT_ID,
        )?;
        signed(
            partial,
            evidence,
            "transition_object/fade_in/time",
            "musicTransitionObjectFadeInTimeBits",
            -21,
        )?;
        assert!(
            partial
                .1
                .data
                .pointer("/payload/transition_object/fade_in/offset")
                .is_none(),
            "the original fade offset does not fit"
        );
        assert!(
            partial
                .1
                .data
                .pointer("/payload/transition_object/fade_out")
                .is_none(),
            "the later fade is beyond the truncated offset"
        );
        Ok(())
    })
}

#[test]
fn empty_and_older_transition_tables_have_separate_coverage() -> TestResult {
    let authored = 0_u32.to_le_bytes();
    for (version, expected, state) in [
        (154, HircLayout::Decoded, "decoded"),
        (153, HircLayout::Unsupported, "unsupported"),
    ] {
        let bank = source(Owner::Playlist, version, &authored, true)?;
        export(&bank, false, expected, |rows, evidence| {
            let summary = row(rows, SUMMARY, &[1, 0])?;
            layout(summary.1, state, Status::Complete);
            if version == 154 {
                evidence.word(
                    summary,
                    "observed/declared_count",
                    "musicTransitionRuleCount",
                    0,
                )?;
            } else {
                assert!(
                    summary.1.data.pointer("/payload/observed").is_none(),
                    "older raw object does not inherit v154 field guesses"
                );
            }
            assert!(
                rows.iter().all(|row| row.subresource.namespace != RULE),
                "there are no admitted rules in either fixture"
            );
            Ok(())
        })?;
    }
    Ok(())
}
