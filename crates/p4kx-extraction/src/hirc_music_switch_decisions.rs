// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_audio::BankMusicSwitchFields;
use schemars::JsonSchema;
use serde::Serialize;

use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

#[path = "hirc_music_switch_decision_rows.rs"]
mod rows;

use self::rows::{DecisionRow, Rows};
use super::{DocumentSite, Layout, Owner};

pub(in crate::hirc_view) type MusicSwitchDecisionsSchema = Decisions<Vec<DecisionRow<'static>>>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MusicSwitchDecisions")]
pub(in crate::hirc_view) struct Decisions<Rows> {
    layout: SourcedValue<DecisionLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    continue_playback_code: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tree_size: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tree_mode_code: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tree_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    targets_compact: Option<SourcedValue<bool>>,
    #[schemars(with = "Vec<DecisionRow<'static>>")]
    rows: Rows,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum DecisionLayout {
    Decoded,
    Malformed,
    Unavailable,
    Unsupported,
}

pub(super) fn observed<'view, 'input, R>(
    site: &DocumentSite<'view>,
    layout: &Layout,
    fields: &BankMusicSwitchFields,
    owner: &'view RefCell<Owner<'input, R>>,
) -> Result<Decisions<Rows<'view, 'input, R>>, SourceEvidenceError> {
    let origins = site.origins;
    let rows = Rows::new(owner, origins, fields);
    let Some(playback) = &fields.continue_playback_code else {
        return unobserved(site, layout, rows);
    };
    let end = fields
        .tree_range
        .as_ref()
        .map_or_else(|| site.original.body_end_exclusive(), |range| range.end);
    let extent = playback.range.start..end;
    let layout = if fields.tree_complete {
        DecisionLayout::Decoded
    } else {
        DecisionLayout::Malformed
    };
    Ok(Decisions {
        layout: origins.derived(layout, &extent, "musicSwitchDecisionCoverage")?,
        continue_playback_code: Some(origins.field(playback, "musicSwitchContinuePlayback")?),
        group_count: origins.optional(fields.group_count.as_ref(), "musicSwitchGroupCount")?,
        tree_size: origins.optional(fields.tree_size.as_ref(), "musicSwitchTreeSize")?,
        tree_mode_code: origins.optional(fields.tree_mode_code.as_ref(), "musicSwitchTreeMode")?,
        tree_range: fields
            .tree_range
            .as_ref()
            .map(|range| origins.range(range, "musicSwitchTreeExtent"))
            .transpose()?,
        targets_compact: fields
            .targets_compact
            .then(|| origins.derived(true, &extent, "musicSwitchTargetsCompact"))
            .transpose()?,
        rows,
    })
}

fn unobserved<Rows>(
    site: &DocumentSite<'_>,
    layout: &Layout,
    rows: Rows,
) -> Result<Decisions<Rows>, SourceEvidenceError> {
    let layout = if matches!(layout, Layout::Unsupported) {
        DecisionLayout::Unsupported
    } else {
        DecisionLayout::Unavailable
    };
    Ok(Decisions {
        layout: site.origins.coverage(layout, site.header, site.original)?,
        continue_playback_code: None,
        group_count: None,
        tree_size: None,
        tree_mode_code: None,
        tree_range: None,
        targets_compact: None,
        rows,
    })
}

pub(super) fn unsupported(
    site: &DocumentSite<'_>,
) -> Result<MusicSwitchDecisionsSchema, SourceEvidenceError> {
    unobserved(site, &Layout::Unsupported, Vec::new())
}
