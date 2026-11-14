// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::HircLayout;
use p4kx_media::Status;
use serde_json::json;

use super::super::support::audio_provider::TestResult;
use super::fixture::{Body, Node, bank, export};
use super::{PATH, POSITIONING, layout, row};

#[test]
fn inherited_static_and_empty_automated_positioning_remain_distinct() -> TestResult {
    let mut empty_automation = vec![0x23, 0, 0];
    empty_automation.extend([0_u32; 3].into_iter().flat_map(u32::to_le_bytes));
    for (bytes, override_parent, spatial, automated) in [
        (&[0x02][..], false, false, false),
        (&[0x03, 0xFF][..], true, true, false),
        (empty_automation.as_slice(), true, true, true),
    ] {
        let source = bank(Node::Sound, 154, Body::Complete(bytes));
        export(&source, false, HircLayout::Decoded, |records, evidence| {
            let summary = row(records, POSITIONING, &[1, 0])?;
            layout(summary.1, "decoded", Status::Complete);
            let data = &summary.1.data;
            assert_eq!(
                data.pointer("/payload/observed/modes/override_parent"),
                Some(&json!(override_parent)),
                "original override control distinguishes inherited positioning",
            );
            assert_eq!(
                data.pointer("/payload/observed/spatial_flags").is_some(),
                spatial,
                "the extra 3D byte requires both controlling flags",
            );
            if automated {
                evidence.scalar(
                    summary,
                    "observed/vertex_count",
                    "nodePositioningVertexCount",
                    0,
                    &0_u32.to_le_bytes(),
                )?;
                evidence.scalar(
                    summary,
                    "observed/playlist_count",
                    "nodePositioningPlaylistCount",
                    0,
                    &0_u32.to_le_bytes(),
                )?;
            } else {
                assert!(
                    data.pointer("/payload/observed/path_mode").is_none(),
                    "static or inherited positioning carries no authored automation mode",
                );
            }
            assert_eq!(
                records
                    .iter()
                    .filter(|record| record.subresource.namespace == PATH)
                    .count(),
                0,
                "empty path tables contribute zero rows",
            );
            Ok(())
        })?;
    }
    Ok(())
}

#[test]
fn partial_playlist_count_does_not_invent_path_or_automation_tables() -> TestResult {
    let full = super::fixture::positioning();
    let playlist_count_start = full.len() - 6 * size_of::<u32>();
    let prefix = full
        .get(..playlist_count_start + size_of::<u16>())
        .ok_or("partial playlist count")?;
    let source = bank(Node::ActorMixer, 154, Body::PositioningPrefix(prefix));
    export(
        &source,
        false,
        HircLayout::Malformed,
        |records, evidence| {
            let summary = row(records, POSITIONING, &[1, 0])?;
            layout(summary.1, "malformed", Status::Partial);
            evidence.scalar(
                summary,
                "observed/vertex_count",
                "nodePositioningVertexCount",
                2,
                &2_u32.to_le_bytes(),
            )?;
            layout(
                row(records, PATH, &[1, 0, 0, 1])?.1,
                "decoded",
                Status::Complete,
            );
            for field in ["playlist_count", "playlist_range", "automation_range"] {
                assert!(
                    summary
                        .1
                        .data
                        .pointer(&format!("/payload/observed/{field}"))
                        .is_none(),
                    "a truncated count establishes no {field}",
                );
            }
            Ok(())
        },
    )
}

#[test]
fn partial_automation_range_keeps_earlier_rows_and_exact_xy_prefix() -> TestResult {
    let full = super::fixture::positioning();
    let prefix = full
        .get(..full.len() - size_of::<u16>())
        .ok_or("partial range Z word")?;
    let source = bank(Node::Sound, 154, Body::PositioningPrefix(prefix));
    export(&source, true, HircLayout::Malformed, |records, evidence| {
        let summary = row(records, POSITIONING, &[1, 0])?;
        layout(summary.1, "malformed", Status::Partial);
        layout(
            row(records, PATH, &[1, 0, 1, 0])?.1,
            "decoded",
            Status::Complete,
        );
        let range = row(records, PATH, &[1, 0, 2, 0])?;
        layout(range.1, "malformed", Status::Partial);
        evidence.scalar(
            range,
            "fields/x/bits",
            "nodePathRangeXBits",
            0x3F80_0000,
            &0x3F80_0000_u32.to_le_bytes(),
        )?;
        evidence.scalar(
            range,
            "fields/y/bits",
            "nodePathRangeYBits",
            0x3F00_0000,
            &0x3F00_0000_u32.to_le_bytes(),
        )?;
        assert!(
            range.1.data.pointer("/payload/fields/z").is_none(),
            "partial Z remains original raw bytes"
        );
        Ok(())
    })
}
