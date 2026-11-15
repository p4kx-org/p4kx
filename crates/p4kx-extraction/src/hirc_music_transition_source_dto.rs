// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::hirc_view::StateRange;
use crate::source_value::SourcedValue;

pub(in crate::hirc_view) type TransitionRulesSchema = TransitionRules<Vec<TransitionRow<'static>>>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MusicNodeTransitionRules")]
pub(in crate::hirc_view) struct TransitionRules<Rows> {
    pub(super) layout: SourcedValue<TransitionLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) declared_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) complete_rules: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) complete_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) checked_range: Option<SourcedValue<StateRange>>,
    #[schemars(with = "Vec<TransitionRow<'static>>")]
    pub(super) rows: Rows,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TransitionLayout {
    Decoded,
    Malformed,
    Unavailable,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum TransitionRow<'row> {
    Rule(&'row Rule),
    Id(&'row ReferenceWord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct Rule {
    pub(super) ordinal: SourcedValue<u32>,
    pub(super) checked_range: SourcedValue<StateRange>,
    pub(super) complete: SourcedValue<bool>,
    pub(super) source_ids: IdTable,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) destination_ids: Option<IdTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<TransitionSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) destination: Option<TransitionDestination>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) allocation_flag: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) transition_object: Option<TransitionObject>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct IdTable {
    pub(super) declared_count: SourcedValue<u32>,
    pub(super) checked_range: SourcedValue<StateRange>,
    pub(super) complete: SourcedValue<bool>,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Side {
    Source,
    Destination,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct ReferenceWord {
    pub(super) rule_ordinal: SourcedValue<u32>,
    pub(super) side: SourcedValue<Side>,
    pub(super) ordinal: SourcedValue<u32>,
    pub(super) id: SourcedValue<u32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct TransitionSource {
    pub(super) fade: Fade,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sync_type: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cue_filter_hash: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) play_post_exit: Option<SourcedValue<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct TransitionDestination {
    pub(super) fade: Fade,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cue_filter_hash: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) jump_to_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) jump_to_type: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) entry_type: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) play_pre_entry: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) match_source_cue_name: Option<SourcedValue<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct TransitionObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) segment_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) fade_in: Option<Fade>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) fade_out: Option<Fade>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) play_pre_entry: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) play_post_exit: Option<SourcedValue<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Fade {
    pub(super) time: SignedWord,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) curve: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) offset: Option<SignedWord>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SignedWord {
    pub(super) bits: SourcedValue<u32>,
    pub(super) signed_value: SourcedValue<i32>,
}
