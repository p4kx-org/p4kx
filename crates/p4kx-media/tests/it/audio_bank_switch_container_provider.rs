// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::audio_bank_container_node_provider::fixture::{Evidence, export, layout, source};
use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

const SWITCH_TYPE: u8 = 6;
const OBJECT_ID: u32 = 0x52C3_1122;
const GROUP_ID: u32 = 0x9281_2030;
const CHILD_A: u32 = 0xA225_3040;
const CHILD_B: u32 = 0xB338_5060;
const SUMMARY: &str = "audio-bank-hirc-switch-container";
const GROUP: &str = "audio-bank-hirc-switch-membership-group";
const MEMBER: &str = "audio-bank-hirc-switch-membership";
const CONFIG: &str = "audio-bank-hirc-switch-child-configuration";

#[test]
fn v154_switch_retains_default_membership_and_ordered_signed_fades() -> TestResult {
    let mut tail = heading();
    append_words(&mut tail, [2, 0, 3, CHILD_B, CHILD_A, CHILD_B, 0, 0, 2]);
    add_configuration(&mut tail, CHILD_A, 0xE7, i32::MIN, -17);
    add_configuration(&mut tail, CHILD_A, 0x03, 0, i32::MAX);
    let bytes = source(SWITCH_TYPE, OBJECT_ID, 154, &tail)?;
    export(&bytes, true, HircLayout::Decoded, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "decoded", Status::Complete);
        heading_rows(summary, evidence)?;
        table_coverage(summary.1, "groups", "complete");
        table_coverage(summary.1, "configurations", "complete");
        evidence.word(
            summary,
            "observed/groups/declared_count",
            "switchMembershipGroupCount",
            2,
        )?;
        evidence.word(
            summary,
            "observed/configurations/declared_count",
            "switchConfigurationCount",
            2,
        )?;
        membership_rows(rows, evidence)?;
        configuration_rows(rows, evidence)
    })
}

fn heading_rows(summary: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    evidence.byte(summary, "observed/group_type", "switchGroupType", 0xFE)?;
    evidence.word(summary, "observed/group_id", "switchGroupId", GROUP_ID)?;
    assert!(
        summary
            .1
            .data
            .pointer("/payload/observed/group_kind")
            .is_none(),
        "unknown group type remains observable without an invented interpretation"
    );
    evidence.word(
        summary,
        "observed/default_value_id",
        "switchDefaultValueId",
        0,
    )?;
    evidence.byte(
        summary,
        "observed/continuous_validation",
        "switchContinuousValidation",
        0xB0,
    )
}

fn membership_rows(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (ordinal, count) in [(0, 3), (1, 0)] {
        let group = row(rows, GROUP, &[1, 0, ordinal])?;
        layout(group.1, "decoded", Status::Complete);
        evidence.word(group, "value_id", "switchMembershipValueId", 0)?;
        evidence.word(group, "member_count", "switchMembershipCount", count)?;
    }
    for (ordinal, child_id) in [(0, CHILD_B), (1, CHILD_A), (2, CHILD_B)] {
        let member = row(rows, MEMBER, &[1, 0, 0, ordinal])?;
        evidence.word(member, "child_id", "switchMembershipChildId", child_id)?;
        let group = row(rows, GROUP, &[1, 0, 0])?.1;
        evidence.index(
            member.0,
            2,
            group,
            "value_id",
            "switchMembershipGroupPosition",
        )?;
        evidence.index(
            member.0,
            3,
            member.1,
            "child_id",
            "switchMembershipPosition",
        )?;
        assert_eq!(
            member.1.data.pointer("/payload/reference_resolution"),
            Some(&json!("unchecked")),
            "authored membership IDs do not select playback or prove a resolved target"
        );
    }
    let legacy = row(rows, "audio-bank-hirc-container-child", &[1, 0, 0])?.1;
    assert_eq!(
        legacy.data.pointer("/payload/child_id"),
        Some(&json!(CHILD_A)),
        "original child order is independent of Switch membership order"
    );
    Ok(())
}

fn configuration_rows(rows: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    for (ordinal, flags, fade_out, fade_in) in [(0, 0xE7, i32::MIN, -17), (1, 0x03, 0, i32::MAX)] {
        let config = row(rows, CONFIG, &[1, 0, ordinal])?;
        layout(config.1, "decoded", Status::Complete);
        evidence.word(config, "child_id", "switchConfigurationChildId", CHILD_A)?;
        evidence.byte(config, "flags", "switchConfigurationFlags", flags)?;
        fade(
            config,
            evidence,
            "fade_out",
            "switchFadeOutBits",
            "switchSignedFadeOut",
            fade_out,
        )?;
        fade(
            config,
            evidence,
            "fade_in",
            "switchFadeInBits",
            "switchSignedFadeIn",
            fade_in,
        )?;
    }
    Ok(())
}

fn fade(
    record: (usize, &MediaRecord),
    evidence: &Evidence<'_>,
    field: &str,
    payload_label: &str,
    conversion_label: &str,
    value: i32,
) -> TestResult {
    evidence.word(
        record,
        &format!("{field}/bits"),
        payload_label,
        u32::from_le_bytes(value.to_le_bytes()),
    )?;
    evidence.conversion(
        record,
        &format!("{field}/signed_value"),
        &format!("{field}/bits"),
        conversion_label,
    )?;
    assert_eq!(
        record
            .1
            .data
            .pointer(&format!("/payload/{field}/signed_value")),
        Some(&json!(value)),
        "the original i32 interpretation preserves the sign"
    );
    Ok(())
}

#[test]
fn malformed_membership_count_keeps_the_checked_member_prefix() -> TestResult {
    let mut tail = heading();
    append_words(&mut tail, [1, 0, u32::MAX, CHILD_B]);
    tail.extend([0xCE, 0xFD]);
    let bytes = source(SWITCH_TYPE, OBJECT_ID, 154, &tail)?;
    export(&bytes, false, HircLayout::Malformed, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.word(
            summary,
            "observed/groups/declared_count",
            "switchMembershipGroupCount",
            1,
        )?;
        let group = row(rows, GROUP, &[1, 0, 0])?;
        layout(group.1, "malformed", Status::Partial);
        evidence.word(group, "member_count", "switchMembershipCount", u32::MAX)?;
        let member = row(rows, MEMBER, &[1, 0, 0, 0])?;
        evidence.word(member, "child_id", "switchMembershipChildId", CHILD_B)?;
        table_coverage(summary.1, "groups", "truncated");
        assert_eq!(count_rows(rows, MEMBER), 1);
        assert_eq!(count_rows(rows, CONFIG), 0);
        assert!(
            summary
                .1
                .data
                .pointer("/payload/observed/configurations")
                .is_none(),
            "configuration table is beyond the truncated membership"
        );
        Ok(())
    })
}

#[test]
fn partial_membership_group_header_keeps_its_readable_value_id() -> TestResult {
    let mut tail = heading();
    append_words(&mut tail, [u32::MAX, 0xF900_2233]);
    tail.extend([0xA5, 0xFB]);
    let bytes = source(SWITCH_TYPE, OBJECT_ID, 154, &tail)?;
    export(&bytes, true, HircLayout::Malformed, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        evidence.word(
            summary,
            "observed/groups/declared_count",
            "switchMembershipGroupCount",
            u32::MAX,
        )?;
        let group = row(rows, GROUP, &[1, 0, 0])?;
        layout(group.1, "malformed", Status::Partial);
        evidence.word(group, "value_id", "switchMembershipValueId", 0xF900_2233)?;
        assert!(group.1.data.pointer("/payload/member_count").is_none());
        assert_eq!(count_rows(rows, GROUP), 1);
        assert_eq!(count_rows(rows, MEMBER), 0);
        Ok(())
    })
}

#[test]
fn malformed_configuration_preserves_independent_fade_out_and_flags() -> TestResult {
    let mut tail = heading();
    append_words(&mut tail, [0, u32::MAX, CHILD_A]);
    tail.push(0xD9);
    tail.extend((-9_i32).to_le_bytes());
    tail.extend([0xCE, 0xFD]);
    let bytes = source(SWITCH_TYPE, OBJECT_ID, 154, &tail)?;
    export(&bytes, false, HircLayout::Malformed, |rows, evidence| {
        let summary = row(rows, SUMMARY, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.word(
            summary,
            "observed/groups/declared_count",
            "switchMembershipGroupCount",
            0,
        )?;
        evidence.word(
            summary,
            "observed/configurations/declared_count",
            "switchConfigurationCount",
            u32::MAX,
        )?;
        let config = row(rows, CONFIG, &[1, 0, 0])?;
        layout(config.1, "malformed", Status::Partial);
        evidence.word(config, "child_id", "switchConfigurationChildId", CHILD_A)?;
        evidence.byte(config, "flags", "switchConfigurationFlags", 0xD9)?;
        fade(
            config,
            evidence,
            "fade_out",
            "switchFadeOutBits",
            "switchSignedFadeOut",
            -9,
        )?;
        assert!(config.1.data.pointer("/payload/fade_in").is_none());
        table_coverage(summary.1, "groups", "complete");
        table_coverage(summary.1, "configurations", "truncated");
        assert_eq!(count_rows(rows, CONFIG), 1);
        Ok(())
    })
}

#[test]
fn empty_and_unsupported_switch_layouts_are_explicit() -> TestResult {
    let mut tail = heading();
    *tail.first_mut().ok_or("original Switch group type")? = 1;
    append_words(&mut tail, [0, 0]);
    for (version, status, expected) in [
        (154, HircLayout::Decoded, "decoded"),
        (153, HircLayout::Unsupported, "unsupported"),
    ] {
        let bytes = source(SWITCH_TYPE, OBJECT_ID, version, &tail)?;
        export(&bytes, false, status, |rows, evidence| {
            let summary = row(rows, SUMMARY, &[1, 0])?;
            layout(summary.1, expected, Status::Complete);
            if version == 154 {
                assert_eq!(
                    summary.1.data.pointer("/payload/observed/group_kind"),
                    Some(&json!("state")),
                    "the original known group type identifies State"
                );
                evidence.conversion(
                    summary,
                    "observed/group_kind",
                    "observed/group_type",
                    "switchGroupKind",
                )?;
                evidence.word(
                    summary,
                    "observed/groups/declared_count",
                    "switchMembershipGroupCount",
                    0,
                )?;
                evidence.word(
                    summary,
                    "observed/configurations/declared_count",
                    "switchConfigurationCount",
                    0,
                )?;
            } else {
                assert!(summary.1.data.pointer("/payload/observed").is_none());
            }
            for namespace in [GROUP, MEMBER, CONFIG] {
                assert_eq!(count_rows(rows, namespace), 0);
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn table_coverage(record: &MediaRecord, table: &str, expected: &str) {
    assert_eq!(
        record
            .data
            .pointer(&format!("/payload/observed/{table}/coverage")),
        Some(&json!(expected)),
        "independent checked coverage of the original {table} list",
    );
}

fn count_rows(rows: &[MediaRecord], namespace: &str) -> usize {
    rows.iter()
        .filter(|row| row.subresource.namespace == namespace)
        .count()
}

fn heading() -> Vec<u8> {
    let mut tail = vec![0xFE];
    append_words(&mut tail, [GROUP_ID, 0]);
    tail.push(0xB0);
    append_words(&mut tail, [2, CHILD_A, CHILD_B]);
    tail
}

fn append_words(tail: &mut Vec<u8>, words: impl IntoIterator<Item = u32>) {
    tail.extend(words.into_iter().flat_map(u32::to_le_bytes));
}

fn add_configuration(tail: &mut Vec<u8>, child_id: u32, flags: u8, fade_out: i32, fade_in: i32) {
    tail.extend(child_id.to_le_bytes());
    tail.push(flags);
    tail.extend(fade_out.to_le_bytes());
    tail.extend(fade_in.to_le_bytes());
}
