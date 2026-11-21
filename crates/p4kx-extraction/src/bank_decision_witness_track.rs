// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_audio::{BankHircField, BankHircMusicTrack, BankMusicTrackClip, BankMusicTrackRow};
use serde_json::{Value, json};

use super::super::values::{Projection, Rows};
use super::super::{Family, Input, TestResult};
use super::transition;

pub(in super::super) fn track(
    node: &mut BankHircMusicTrack<'_, Input<'_>>,
    projection: &Projection<'_>,
    missing: &mut BTreeSet<Family>,
) -> TestResult {
    let mut rows = projection.rows("/rows")?;
    let first_switch = source_and_clips(node, projection, &mut rows)?;
    if automation(node, projection, &mut rows)? {
        missing.remove(&Family::TrackAutomation);
    }
    if let Some((ordinal, id)) = first_switch {
        member(projection, &mut rows, ordinal, &id)?;
    }
    for original in &mut node.rows {
        let BankMusicTrackRow::Switch { ordinal, id } = original? else {
            return Err("ordered Track continuation contains only switch members".into());
        };
        member(projection, &mut rows, ordinal, &id)?;
    }
    rows.finish()?;
    if switch_transition(node, projection)? {
        missing.remove(&Family::TrackTransition);
    }
    Ok(())
}

fn source_and_clips(
    node: &mut BankHircMusicTrack<'_, Input<'_>>,
    projection: &Projection<'_>,
    rows: &mut Rows<'_>,
) -> TestResult<Option<(u32, BankHircField<u32>)>> {
    for original in &mut node.rows {
        match original? {
            BankMusicTrackRow::Source { ordinal, value } => {
                let row = rows.kind("media-source")?;
                projection.at(row, "/ordinal", &json!(ordinal))?;
                projection.optional(row, "/source_id", value.fields.source_id.as_ref())?;
                projection.optional(row, "/cache_id", value.fields.cache_id.as_ref())?;
            }
            BankMusicTrackRow::Clip { ordinal, value } => {
                clip(projection, rows.kind("clip")?, ordinal, &value)?;
            }
            BankMusicTrackRow::Switch { ordinal, id } => return Ok(Some((ordinal, id))),
            _ => return Err("known original Track row".into()),
        }
    }
    Ok(None)
}

fn clip(
    projection: &Projection<'_>,
    row: &Value,
    ordinal: u32,
    value: &BankMusicTrackClip,
) -> TestResult {
    projection.at(row, "/ordinal", &json!(ordinal))?;
    projection.field(row, "/track_id", &value.track_id)?;
    projection.field(row, "/source_id", &value.source_id)?;
    projection.field(row, "/event_id", &value.event_id)?;
    for (path, word) in [
        ("/play_at/bits", &value.play_at),
        ("/begin_trim/bits", &value.begin_trim),
        ("/end_trim/bits", &value.end_trim),
        ("/source_duration/bits", &value.source_duration),
    ] {
        projection.field(row, path, word)?;
    }
    Ok(())
}

fn automation(
    node: &mut BankHircMusicTrack<'_, Input<'_>>,
    projection: &Projection<'_>,
    rows: &mut Rows<'_>,
) -> TestResult<bool> {
    let count = node.fields.automation_count.as_ref();
    projection.optional(
        projection.document,
        "/fields/automation/declared_count",
        count,
    )?;
    let mut point_observed = false;
    let Some(mut recorded) = node.automations() else {
        return Ok(false);
    };
    while let Some(mut original) = recorded.next_automation()? {
        let row = rows.kind("automation")?;
        let ordinal = original.ordinal;
        projection.at(row, "/ordinal", &json!(ordinal))?;
        projection.field(row, "/clip_index", &original.fields.clip_index)?;
        projection.optional(
            row,
            "/automation_type",
            original.fields.automation_type.as_ref(),
        )?;
        let count = original
            .fields
            .points
            .as_ref()
            .map(|points| &points.declared_count);
        projection.optional(row, "/points/declared_count", count)?;
        if let Some(points) = original.points() {
            for point in points {
                let point = point?;
                let row = rows.kind("automation-point")?;
                projection.at(row, "/automation_ordinal", &json!(ordinal))?;
                projection.at(row, "/point_ordinal", &json!(point.ordinal))?;
                projection.field(row, "/from_bits", &point.from_bits)?;
                projection.optional(row, "/to_bits", point.to_bits.as_ref())?;
                projection.optional(row, "/interpolation", point.interpolation.as_ref())?;
                point_observed = true;
            }
        }
    }
    Ok(point_observed)
}

fn member(
    projection: &Projection<'_>,
    rows: &mut Rows<'_>,
    ordinal: u32,
    id: &BankHircField<u32>,
) -> TestResult {
    let row = rows.kind("switch-member")?;
    projection.at(row, "/ordinal", &json!(ordinal))?;
    projection.field(row, "/value_id", id)
}

fn switch_transition(
    node: &BankHircMusicTrack<'_, Input<'_>>,
    projection: &Projection<'_>,
) -> TestResult<bool> {
    let Some(fields) = &node.fields.switch else {
        return Ok(false);
    };
    let row = projection
        .document
        .pointer("/fields/switch")
        .ok_or("original Track switch fields")?;
    projection.optional(row, "/group_type", fields.group_type.as_ref())?;
    projection.optional(row, "/group_id", fields.group_id.as_ref())?;
    projection.optional(row, "/default_value_id", fields.default_switch.as_ref())?;
    projection.optional(row, "/member_count", fields.member_count.as_ref())?;
    let Some(transition_fields) = &fields.transition else {
        return Ok(false);
    };
    let row = row.get("transition").ok_or("original Track transition")?;
    transition::fade(projection, row, "/source_fade", &transition_fields.source)?;
    transition::fade(
        projection,
        row,
        "/destination_fade",
        &transition_fields.destination,
    )?;
    projection.optional(row, "/sync_type", transition_fields.sync_type.as_ref())?;
    projection.optional(
        row,
        "/cue_filter_hash",
        transition_fields.cue_filter_hash.as_ref(),
    )?;
    Ok(fields.transition_range.is_some())
}
