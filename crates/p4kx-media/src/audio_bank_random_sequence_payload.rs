// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::{NodeDeclaration, NodeReferenceStatus};
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const SUMMARY_NAMESPACE: &str = "audio-bank-hirc-random-sequence";
pub(crate) const ITEM_NAMESPACE: &str = "audio-bank-hirc-random-sequence-playlist-item";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum RandomSequencePayload<'row> {
    #[serde(rename = "bank-hirc-random-sequence")]
    Summary(&'row NodeDeclaration<RandomSequenceObservations>),
    #[serde(rename = "bank-hirc-random-sequence-playlist-item")]
    Item(&'row RandomSequenceItemRecord),
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RandomSequenceObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) loop_count: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) loop_minimum: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) loop_maximum: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transition_time: Option<TransitionWord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transition_minimum: Option<TransitionWord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transition_maximum: Option<TransitionWord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) avoid_repeat_count: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transition_mode: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) random_mode: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) playlist_count: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) playlist_range: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TransitionWord {
    pub(crate) bits: BankField<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finite_value: Option<f32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum RandomSequenceItemRecord {
    Decoded {
        ordinal: u16,
        play_id: BankField<u32>,
        weight_bits: BankField<u32>,
        weight: i32,
        reference_resolution: NodeReferenceStatus,
    },
    Malformed {
        ordinal: u16,
        play_id: BankField<u32>,
        reference_resolution: NodeReferenceStatus,
    },
}
