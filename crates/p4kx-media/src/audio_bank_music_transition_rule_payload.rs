// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const SUMMARY: &str = "audio-bank-music-transition-rules";
pub(crate) const RULE: &str = "audio-bank-music-transition-rule";
pub(crate) const ID: &str = "audio-bank-music-transition-id";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum MusicTransitionRulePayload<'row> {
    #[serde(rename = "bank-music-transition-rules")]
    Summary(&'row NodeDeclaration<TransitionRuleSummary>),
    #[serde(rename = "bank-music-transition-rule")]
    Rule(&'row TransitionRuleRecord),
    #[serde(rename = "bank-music-transition-id")]
    Id(&'row TransitionIdRecord),
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionRuleSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) declared_count: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) complete_rules: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) checked_range: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionRuleRecord {
    pub(crate) layout: TransitionRuleLayout,
    pub(crate) source_ids: TransitionIdTable,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) destination_ids: Option<TransitionIdTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source: Option<TransitionSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) destination: Option<TransitionDestination>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) allocation_flag: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transition_object: Option<TransitionObject>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TransitionRuleLayout {
    Decoded,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionIdTable {
    pub(crate) declared_count: BankField<u32>,
    pub(crate) checked_range: BankEntryRange,
    pub(crate) coverage: TransitionIdCoverage,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TransitionIdCoverage {
    Complete,
    Truncated,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionIdRecord {
    pub(crate) list: TransitionIdSide,
    pub(crate) id: BankField<u32>,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum TransitionIdSide {
    Source,
    Destination,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionSource {
    pub(crate) fade: TransitionFade,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sync_type: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cue_filter_hash: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) play_post_exit: Option<BankField<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionDestination {
    pub(crate) fade: TransitionFade,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cue_filter_hash: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) jump_to_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) jump_to_type: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entry_type: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) play_pre_entry: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) match_source_cue_name: Option<BankField<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionObject {
    pub(crate) segment_id: BankField<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fade_in: Option<TransitionFade>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fade_out: Option<TransitionFade>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) play_pre_entry: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) play_post_exit: Option<BankField<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionFade {
    pub(crate) time: TransitionSignedWord,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) curve: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) offset: Option<TransitionSignedWord>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionSignedWord {
    pub(crate) bits: BankField<u32>,
    pub(crate) signed_value: i32,
}
