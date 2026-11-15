// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    BankHircField, BankMusicFadeFields, BankMusicTransitionObjectFields, BankMusicTransitionRows,
    BankMusicTransitionRuleFields, BankMusicTransitionSide,
};
use serde_json::{Value, json};

use super::super::values::Projection;
use super::super::{Input, TestResult, require};

pub(super) fn check(
    projection: &Projection<'_>,
    declared: Option<&BankHircField<u32>>,
    complete: u32,
    source: Option<BankMusicTransitionRows<'_, Input<'_>>>,
) -> TestResult<bool> {
    let table = projection
        .document
        .get("transition_rules")
        .ok_or("transition table summary")?;
    projection.optional(table, "/declared_count", declared)?;
    if declared.is_some() {
        projection.at(table, "/complete_rules", &json!(complete))?;
    }
    let mut rows = projection.rows("/transition_rules/rows")?;
    if let Some(mut source) = source {
        while let Some(mut original) = source.next_rule()? {
            let row = rows.kind("rule")?;
            projection.at(row, "/ordinal", &json!(original.ordinal))?;
            projection.at(row, "/complete", &json!(original.issue.is_none()))?;
            rule_fields(projection, row, &original.fields)?;
            let rule_ordinal = original.ordinal;
            for (side, name) in [
                (BankMusicTransitionSide::Source, "source"),
                (BankMusicTransitionSide::Destination, "destination"),
            ] {
                if let Some(ids) = original.ids(side) {
                    for id in ids {
                        let id = id?;
                        let row = rows.kind("id")?;
                        projection.at(row, "/rule_ordinal", &json!(rule_ordinal))?;
                        projection.at(row, "/side", &json!(name))?;
                        projection.at(row, "/ordinal", &json!(id.ordinal))?;
                        projection.field(row, "/id", &id.id)?;
                    }
                }
            }
        }
    }
    rows.finish()?;
    Ok(complete > 0)
}

fn rule_fields(
    projection: &Projection<'_>,
    row: &Value,
    original: &BankMusicTransitionRuleFields,
) -> TestResult {
    projection.field(
        row,
        "/source_ids/declared_count",
        &original.source_ids.declared_count,
    )?;
    projection.optional(
        row,
        "/destination_ids/declared_count",
        original
            .destination_ids
            .as_ref()
            .map(|ids| &ids.declared_count),
    )?;
    projection.optional(row, "/allocation_flag", original.allocation_flag.as_ref())?;
    let source = &original.source;
    let destination = &original.destination;
    fade(projection, row, "/source/fade", &source.fade)?;
    fade(projection, row, "/destination/fade", &destination.fade)?;
    for (path, word) in [
        ("/source/sync_type", source.sync_type.as_ref()),
        ("/source/cue_filter_hash", source.cue_filter_hash.as_ref()),
        (
            "/destination/cue_filter_hash",
            destination.cue_filter_hash.as_ref(),
        ),
        ("/destination/jump_to_id", destination.jump_to_id.as_ref()),
    ] {
        projection.optional(row, path, word)?;
    }
    projection.optional(
        row,
        "/destination/jump_to_type",
        destination.jump_to_type.as_ref(),
    )?;
    projection.optional(
        row,
        "/destination/entry_type",
        destination.entry_type.as_ref(),
    )?;
    for (path, word) in [
        ("/source/play_post_exit", source.play_post_exit.as_ref()),
        (
            "/destination/play_pre_entry",
            destination.play_pre_entry.as_ref(),
        ),
        (
            "/destination/match_source_cue_name",
            destination.match_source_cue_name.as_ref(),
        ),
    ] {
        projection.optional(row, path, word)?;
    }
    transition_object(projection, row, original.transition_object.as_ref())
}

fn transition_object(
    projection: &Projection<'_>,
    row: &Value,
    object: Option<&BankMusicTransitionObjectFields>,
) -> TestResult {
    if let Some(object) = object {
        projection.optional(
            row,
            "/transition_object/segment_id",
            object.segment_id.as_ref(),
        )?;
        fade(
            projection,
            row,
            "/transition_object/fade_in",
            &object.fade_in,
        )?;
        fade(
            projection,
            row,
            "/transition_object/fade_out",
            &object.fade_out,
        )?;
        projection.optional(
            row,
            "/transition_object/play_pre_entry",
            object.play_pre_entry.as_ref(),
        )?;
        projection.optional(
            row,
            "/transition_object/play_post_exit",
            object.play_post_exit.as_ref(),
        )?;
    } else {
        require(
            row.get("transition_object").is_none(),
            "no fabricated transition object",
        )?;
    }
    Ok(())
}

pub(super) fn fade(
    projection: &Projection<'_>,
    row: &Value,
    path: &str,
    source: &BankMusicFadeFields,
) -> TestResult {
    for (name, word) in [
        ("time", source.time_bits.as_ref()),
        ("offset", source.offset_bits.as_ref()),
    ] {
        projection.optional(row, &format!("{path}/{name}/bits"), word)?;
        if let Some(word) = word {
            projection.at(
                row,
                &format!("{path}/{name}/signed_value"),
                &json!(word.value.cast_signed()),
            )?;
        }
    }
    projection.optional(row, &format!("{path}/curve"), source.curve.as_ref())
}
