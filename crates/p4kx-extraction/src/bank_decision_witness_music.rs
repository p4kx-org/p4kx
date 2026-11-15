// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_audio::{
    BankHircMusicPlaylist, BankHircMusicSegment, BankHircMusicSwitch, BankMusicNodeFields,
    BankMusicPlaylistRow, BankMusicSegmentRow, BankMusicStinger, BankMusicSwitchBranch,
    BankMusicSwitchFields, BankMusicSwitchRow, BankMusicSwitchTarget, BankReadError,
};
use serde_json::{Value, json};

use super::values::Projection;
use super::{Family, Input, TestResult, require};

const DECISION_NODE_BYTES: u32 = 12;

#[path = "bank_decision_witness_track.rs"]
mod track;
#[path = "bank_decision_witness_transition.rs"]
mod transition;

pub(super) use self::track::track;

pub(super) fn segment(
    node: BankHircMusicSegment<'_, Input<'_>>,
    projection: &Projection<'_>,
    missing: &mut BTreeSet<Family>,
) -> TestResult {
    let family = Family::SegmentStinger;
    if stingers(projection, &node.fields.node, node.rows, |row| match row {
        BankMusicSegmentRow::Stinger { ordinal, value } => Some((ordinal, value)),
        _ => None,
    })? {
        missing.remove(&family);
    }
    Ok(())
}

pub(super) fn playlist(
    node: &mut BankHircMusicPlaylist<'_, Input<'_>>,
    projection: &Projection<'_>,
    missing: &mut BTreeSet<Family>,
) -> TestResult {
    if missing.contains(&Family::PlaylistStinger)
        && stingers(
            projection,
            &node.fields.node,
            &mut node.rows,
            |row| match row {
                BankMusicPlaylistRow::Stinger { ordinal, value } => Some((ordinal, value)),
                _ => None,
            },
        )?
    {
        missing.remove(&Family::PlaylistStinger);
    }
    if missing.contains(&Family::PlaylistTransition) {
        let declared = node.fields.transitions.rule_count.clone();
        let complete = node.fields.transitions.complete_rules;
        if transition::check(
            projection,
            declared.as_ref(),
            complete,
            node.transition_rules(),
        )? {
            missing.remove(&Family::PlaylistTransition);
        }
    }
    Ok(())
}

pub(super) fn switch(
    node: &mut BankHircMusicSwitch<'_, Input<'_>>,
    projection: &Projection<'_>,
    missing: &mut BTreeSet<Family>,
) -> TestResult {
    if missing.contains(&Family::SwitchStinger)
        && stingers(
            projection,
            &node.fields.node,
            &mut node.rows,
            |row| match row {
                BankMusicSwitchRow::Stinger { ordinal, value } => Some((ordinal, value)),
                _ => None,
            },
        )?
    {
        missing.remove(&Family::SwitchStinger);
    }
    if missing.contains(&Family::SwitchTransition) {
        let declared = node.fields.transitions.rule_count.clone();
        let complete = node.fields.transitions.complete_rules;
        if transition::check(
            projection,
            declared.as_ref(),
            complete,
            node.transition_rules(),
        )? {
            missing.remove(&Family::SwitchTransition);
        }
    }
    if missing.contains(&Family::MusicDecision) && decisions(projection, node)? {
        missing.remove(&Family::MusicDecision);
    }
    Ok(())
}

fn stingers<Row>(
    projection: &Projection<'_>,
    fields: &BankMusicNodeFields,
    source: impl Iterator<Item = Result<Row, BankReadError>>,
    as_stinger: impl Fn(Row) -> Option<(u32, BankMusicStinger)>,
) -> TestResult<bool> {
    let mut rows = projection.rows("/stingers")?;
    projection.optional(
        projection.document,
        "/stinger_count",
        fields.stinger_count.as_ref(),
    )?;
    let mut complete = 0_u32;
    for source in source {
        if let Some((ordinal, source)) = as_stinger(source?) {
            let row = rows.one()?;
            projection.at(row, "/ordinal", &json!(ordinal))?;
            projection.at(row, "/complete", &json!(true))?;
            for (path, word) in [
                ("/trigger_id", &source.trigger_id),
                ("/segment_id", &source.segment_id),
                ("/sync_play_at", &source.sync_play_at),
                ("/cue_filter_hash", &source.cue_filter_hash),
                ("/dont_repeat_time/bits", &source.dont_repeat_time_bits),
                ("/segment_look_ahead", &source.segment_look_ahead),
            ] {
                projection.field(row, path, word)?;
            }
            complete += 1;
        }
    }
    if let Some(tail) = &fields.stinger_tail {
        let row = rows.one()?;
        projection.at(row, "/ordinal", &json!(tail.ordinal))?;
        projection.at(row, "/complete", &json!(false))?;
        projection.field(row, "/trigger_id", &tail.trigger_id)?;
        for (path, word) in [
            ("/segment_id", tail.segment_id.as_ref()),
            ("/sync_play_at", tail.sync_play_at.as_ref()),
            ("/cue_filter_hash", tail.cue_filter_hash.as_ref()),
            (
                "/dont_repeat_time/bits",
                tail.dont_repeat_time_bits.as_ref(),
            ),
        ] {
            projection.optional(row, path, word)?;
        }
        projection.optional::<u32>(row, "/segment_look_ahead", None)?;
    }
    rows.finish()?;
    Ok(complete != 0)
}

fn decisions(
    projection: &Projection<'_>,
    node: &mut BankHircMusicSwitch<'_, Input<'_>>,
) -> TestResult<bool> {
    decision_header(projection, &node.fields)?;
    let header = &node.fields;
    let (compact, complete) = (header.targets_compact, header.tree_complete);
    let group_count = header.group_count.as_ref().map(|word| word.value);
    let physical_count = header
        .tree_size
        .as_ref()
        .map(|word| word.value / DECISION_NODE_BYTES);
    let mut rows = projection.rows("/decisions/rows")?;
    let mut physical = node.decision_rows();
    let mut observed_branch = false;
    let mut groups = 0;
    let mut nodes = 0;
    while let Some(row) = physical.next() {
        match row? {
            BankMusicSwitchRow::Group { ordinal, value } => {
                let row = rows.kind("group")?;
                groups += 1;
                projection.at(row, "/ordinal", &json!(ordinal))?;
                projection.field(row, "/group_id", &value.id)?;
                projection.optional(row, "/type_code", value.type_code.as_ref())?;
            }
            BankMusicSwitchRow::Node { ordinal, value } => {
                let row = rows.kind("node")?;
                nodes += 1;
                projection.at(row, "/ordinal", &json!(ordinal))?;
                projection.field(row, "/key", &value.key)?;
                projection.field(row, "/link_word", &value.link)?;
                projection.field(row, "/weight", &value.weight)?;
                projection.field(row, "/probability", &value.probability)?;
            }
            BankMusicSwitchRow::Target { ordinal, value } => {
                require(
                    !compact,
                    "compact decisions represent routes through physical rows",
                )?;
                observed_branch |= target(
                    projection,
                    rows.kind("target")?,
                    ordinal,
                    &value,
                    physical.current_target_route(),
                )?;
            }
            _ => return Err("original decision cursor must contain only decision rows".into()),
        }
    }
    rows.finish()?;
    if complete {
        require(
            Some(groups) == group_count && Some(nodes) == physical_count,
            "complete MusicSwitch retains all declared groups and physical nodes",
        )?;
    }
    Ok(complete && (observed_branch || (compact && groups > 0 && nodes > 0)))
}

fn target<'branch>(
    projection: &Projection<'_>,
    row: &Value,
    ordinal: u32,
    source: &BankMusicSwitchTarget,
    original_route: impl Iterator<Item = &'branch BankMusicSwitchBranch>,
) -> TestResult<bool> {
    projection.at(row, "/ordinal", &json!(ordinal))?;
    projection.at(row, "/level", &json!(source.level))?;
    projection.at(row, "/node_index", &json!(source.node_index))?;
    projection.field(row, "/target_id", &source.id)?;
    let ancestors = row
        .get("ancestor_nodes")
        .and_then(Value::as_array)
        .ok_or("ordered target ancestor path")?;
    let mut route = ancestors.iter();
    let target = row.get("target_id").ok_or("sourced target ID")?;
    for branch in original_route {
        let ancestor = route.next().ok_or("every original selecting ancestor")?;
        projection.at(ancestor, "", &json!(branch.node_index))?;
        projection.receipt(ancestor, &branch.link.range, "selector")?;
        projection.receipt(target, &branch.link.range, "selector")?;
    }
    require(route.next().is_none(), "no extra ancestor decisions")?;
    Ok(!ancestors.is_empty())
}

fn decision_header(projection: &Projection<'_>, fields: &BankMusicSwitchFields) -> TestResult {
    let document = projection
        .document
        .get("decisions")
        .ok_or("original MusicSwitch decisions")?;
    let layout = if fields.tree_complete {
        "decoded"
    } else if fields.continue_playback_code.is_some() {
        "malformed"
    } else {
        "unavailable"
    };
    projection.at(document, "/layout", &json!(layout))?;
    projection.optional(document, "/group_count", fields.group_count.as_ref())?;
    projection.optional(
        document,
        "/continue_playback_code",
        fields.continue_playback_code.as_ref(),
    )?;
    projection.optional(document, "/tree_size", fields.tree_size.as_ref())?;
    projection.optional(document, "/tree_mode_code", fields.tree_mode_code.as_ref())?;
    if let Some(tree) = &fields.tree_range {
        let wire = json!({
            "start": tree.start.to_string(),
            "end_exclusive": tree.end.to_string(),
        });
        projection.at(document, "/tree_range", &wire)?;
        projection.receipt(
            document.get("tree_range").ok_or("exact tree range")?,
            tree,
            "selector",
        )?;
    }
    let marker = document.get("targets_compact");
    if fields.targets_compact {
        projection.at(document, "/targets_compact", &json!(true))?;
        let marker = marker.ok_or("sourced compact route marker")?;
        let start = fields
            .continue_playback_code
            .as_ref()
            .ok_or("original decision header")?
            .range
            .start;
        let end = fields.tree_range.as_ref().ok_or("compact tree extent")?.end;
        projection.receipt(marker, &(start..end), "conversion")?;
    } else {
        require(
            marker.is_none(),
            "expanded decisions omit the compact route marker",
        )?;
    }
    Ok(())
}
