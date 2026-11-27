// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::audio_bank_state_provider::row;
use super::support::audio_provider::TestResult;

#[path = "audio_bank_node_positioning_fixture.rs"]
mod fixture;

#[path = "audio_bank_node_positioning_branches.rs"]
mod branches;

use fixture::{ADVANCED, Body, Evidence, Node, bank, export, positioning};

const POSITIONING: &str = "audio-bank-hirc-node-positioning";
const PATH: &str = "audio-bank-hirc-node-positioning-path";
const SETTINGS: &str = "audio-bank-hirc-node-advanced-settings";

#[test]
fn v154_paths_and_advanced_settings_keep_authored_fields_for_both_owners() -> TestResult {
    for (node, encrypted) in [(Node::Sound, true), (Node::ActorMixer, false)] {
        let source = bank(node, 154, Body::Complete(&positioning()));
        export(
            &source,
            encrypted,
            HircLayout::Decoded,
            |records, evidence| {
                assert_positioning(records, evidence)?;
                assert_vertices(records, evidence)?;
                assert_playlist_and_ranges(records, evidence)?;
                assert_settings(records, evidence)
            },
        )?;
    }
    Ok(())
}

fn assert_positioning(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let summary = row(records, POSITIONING, &[1, 0])?;
    layout(summary.1, "decoded", Status::Complete);
    assert_modes(summary, evidence)?;
    evidence.scalar(
        summary,
        "observed/flags",
        "nodePositioningFlags",
        0xE7,
        &[0xE7],
    )?;
    evidence.scalar(
        summary,
        "observed/spatial_flags",
        "nodePositioningSpatialFlags",
        0xB9,
        &[0xB9],
    )?;
    evidence.scalar(
        summary,
        "observed/path_mode",
        "nodePositioningPathMode",
        0xFD,
        &[0xFD],
    )?;
    evidence.scalar(
        summary,
        "observed/transition_time",
        "nodePositioningTransitionTime",
        -123,
        &(-123_i32).to_le_bytes(),
    )?;
    evidence.scalar(
        summary,
        "observed/vertex_count",
        "nodePositioningVertexCount",
        2,
        &2_u32.to_le_bytes(),
    )?;
    evidence.scalar(
        summary,
        "observed/playlist_count",
        "nodePositioningPlaylistCount",
        1,
        &1_u32.to_le_bytes(),
    )?;
    Ok(())
}

fn assert_modes(summary: (usize, &MediaRecord), evidence: &Evidence<'_>) -> TestResult {
    evidence.conversion(
        summary,
        "observed/modes/override_parent",
        "observed/flags",
        "nodePositioningModeBits",
    )?;
    assert_eq!(
        summary.1.data.pointer("/payload/observed/modes"),
        Some(&json!({
            "override_parent": true,
            "has_spatial_fields": true,
            "panner_type_code": 1,
            "position_type_code": 3,
        })),
        "derived codes use the original positioning flag byte",
    );
    Ok(())
}

fn assert_vertices(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let first = row(records, PATH, &[1, 0, 0, 0])?;
    evidence.conversion(
        first,
        "fields/z/finite_value",
        "fields/z/bits",
        "nodePositioningFiniteCoordinate",
    )?;
    layout(first.1, "decoded", Status::Complete);
    evidence.scalar(
        first,
        "fields/x/bits",
        "nodePathVertexXBits",
        0x7FC1_0023,
        &0x7FC1_0023_u32.to_le_bytes(),
    )?;
    evidence.scalar(
        first,
        "fields/y/bits",
        "nodePathVertexYBits",
        0x8000_0000,
        &0x8000_0000_u32.to_le_bytes(),
    )?;
    evidence.scalar(
        first,
        "fields/duration",
        "nodePathVertexDuration",
        -5,
        &(-5_i32).to_le_bytes(),
    )?;
    assert!(
        first
            .1
            .data
            .pointer("/payload/fields/x/finite_value")
            .is_none(),
        "original NaN bits are valid JSON without an invented finite value"
    );
    assert_eq!(
        first.1.data.pointer("/payload/fields/z/finite_value"),
        Some(&json!(1.5)),
        "known finite coordinate"
    );
    let second = row(records, PATH, &[1, 0, 0, 1])?;
    evidence.scalar(
        second,
        "fields/x/bits",
        "nodePathVertexXBits",
        0xFF80_0000,
        &0xFF80_0000_u32.to_le_bytes(),
    )?;
    Ok(())
}

fn assert_playlist_and_ranges(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let playlist = row(records, PATH, &[1, 0, 1, 0])?;
    evidence.scalar(
        playlist,
        "fields/vertices_offset",
        "nodePathVerticesOffset",
        1,
        &1_u32.to_le_bytes(),
    )?;
    evidence.scalar(
        playlist,
        "fields/vertex_count",
        "nodePathPlaylistVertexCount",
        1,
        &1_u32.to_le_bytes(),
    )?;
    let range = row(records, PATH, &[1, 0, 2, 0])?;
    evidence.scalar(
        range,
        "fields/z/bits",
        "nodePathRangeZBits",
        0x7F80_0000,
        &0x7F80_0000_u32.to_le_bytes(),
    )?;
    assert!(
        range
            .1
            .data
            .pointer("/payload/fields/z/finite_value")
            .is_none(),
        "infinite automation range remains raw"
    );
    Ok(())
}

fn assert_settings(records: &[MediaRecord], evidence: &Evidence<'_>) -> TestResult {
    let record = row(records, SETTINGS, &[1, 0])?;
    layout(record.1, "decoded", Status::Complete);
    let [first, queue, low, high, threshold, second] = ADVANCED;
    for (name, label, raw) in [
        ("flags", "nodeAdvancedFlags", first),
        (
            "virtual_queue_behavior",
            "nodeAdvancedVirtualQueueBehavior",
            queue,
        ),
        (
            "below_threshold_behavior",
            "nodeAdvancedBelowThresholdBehavior",
            threshold,
        ),
        ("behavior_flags", "nodeAdvancedBehaviorFlags", second),
    ] {
        evidence.scalar(
            record,
            &format!("observed/{name}"),
            label,
            i64::from(raw),
            &[raw],
        )?;
    }
    evidence.scalar(
        record,
        "observed/max_instances",
        "nodeAdvancedMaxInstances",
        0x1234,
        &[low, high],
    )
}

#[test]
fn declared_maximum_vertices_never_outgrow_the_original_body() -> TestResult {
    let mut prefix = positioning().get(..7).ok_or("exact path header")?.to_vec();
    prefix.extend(u32::MAX.to_le_bytes());
    prefix.extend(0x8000_0000_u32.to_le_bytes());
    prefix.extend([0xAB, 0xCD]);
    let source = bank(Node::Sound, 154, Body::PositioningPrefix(&prefix));
    export(&source, true, HircLayout::Malformed, |records, evidence| {
        let summary = row(records, POSITIONING, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        evidence.scalar(
            summary,
            "observed/vertex_count",
            "nodePositioningVertexCount",
            i64::from(u32::MAX),
            &u32::MAX.to_le_bytes(),
        )?;
        let item = row(records, PATH, &[1, 0, 0, 0])?;
        layout(item.1, "malformed", Status::Partial);
        evidence.scalar(
            item,
            "fields/x/bits",
            "nodePathVertexXBits",
            0x8000_0000,
            &0x8000_0000_u32.to_le_bytes(),
        )?;
        assert!(
            item.1.data.pointer("/payload/fields/y").is_none(),
            "the partial y word has no typed value"
        );
        assert_eq!(
            records
                .iter()
                .filter(|record| record.subresource.namespace == PATH)
                .count(),
            1,
            "only the checked vertex prefix is emitted"
        );
        layout(
            row(records, SETTINGS, &[1, 0])?.1,
            "unavailable",
            Status::Complete,
        );
        Ok(())
    })
}

#[test]
fn truncated_advanced_settings_retain_the_checked_prefix_and_earlier_positioning() -> TestResult {
    let prefix = ADVANCED.get(..3).ok_or("truncated instance limit")?;
    let source = bank(Node::ActorMixer, 154, Body::AdvancedPrefix(prefix));
    export(
        &source,
        false,
        HircLayout::Malformed,
        |records, evidence| {
            let positioning = row(records, POSITIONING, &[1, 0])?;
            layout(positioning.1, "decoded", Status::Complete);
            evidence.scalar(
                positioning,
                "observed/flags",
                "nodePositioningFlags",
                0,
                &[0],
            )?;
            assert!(
                positioning
                    .1
                    .data
                    .pointer("/payload/observed/path_mode")
                    .is_none(),
                "no original path block follows zero positioning flags"
            );
            let settings = row(records, SETTINGS, &[1, 0])?;
            layout(settings.1, "malformed", Status::Partial);
            evidence.scalar(
                settings,
                "observed/flags",
                "nodeAdvancedFlags",
                0xF1,
                &[0xF1],
            )?;
            evidence.scalar(
                settings,
                "observed/virtual_queue_behavior",
                "nodeAdvancedVirtualQueueBehavior",
                0xD5,
                &[0xD5],
            )?;
            assert!(
                settings
                    .1
                    .data
                    .pointer("/payload/observed/max_instances")
                    .is_none(),
                "one byte cannot supply the u16 instance limit"
            );
            Ok(())
        },
    )
}

#[test]
fn absent_sound_and_unsupported_actor_have_explicit_layout_status() -> TestResult {
    for (source, expected, layout_name) in [
        (
            bank(Node::Sound, 154, Body::Absent),
            HircLayout::Decoded,
            "absent",
        ),
        (
            bank(Node::ActorMixer, 153, Body::Complete(&positioning())),
            HircLayout::Unsupported,
            "unsupported",
        ),
    ] {
        export(&source, false, expected, |records, _| {
            for namespace in [POSITIONING, SETTINGS] {
                let record = row(records, namespace, &[1, 0])?.1;
                layout(record, layout_name, Status::Complete);
                assert!(
                    record.data.pointer("/payload/observed").is_none(),
                    "no typed values for {layout_name}"
                );
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn layout(record: &MediaRecord, expected: &str, status: Status) {
    assert_eq!(
        record.status, status,
        "publication status reflects independent original coverage"
    );
    assert_eq!(
        record.data.pointer("/payload/layout"),
        Some(&json!(expected)),
        "reported node layout"
    );
}
