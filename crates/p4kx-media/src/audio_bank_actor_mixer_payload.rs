// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::BankActorMixerFields;
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_payload::{BankEntryRange, BankField};

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum ActorMixerPayload<'row> {
    BankHircActorMixer(&'row ActorMixerRecord),
    BankHircActorMixerChild(&'row ActorMixerChildRecord),
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "HircActorMixerLayout")]
pub(super) enum Layout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActorMixerRecord {
    layout: Layout,
    version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    effect_count: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_count: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pre_parent_word: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_node_range: Option<BankEntryRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    child_count: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_children: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_children_range: Option<BankEntryRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trailing_range: Option<BankEntryRange>,
}

impl ActorMixerRecord {
    pub(super) fn read(layout: Layout, version: u32, fields: &BankActorMixerFields) -> Self {
        Self {
            layout,
            version,
            effect_count: fields.node.effect_count.as_ref().map(BankField::from),
            metadata_count: fields.node.metadata_count.as_ref().map(BankField::from),
            pre_parent_word: fields.node.pre_parent_word.as_ref().map(BankField::from),
            parent_id: fields.node.parent_id.as_ref().map(BankField::from),
            checked_node_range: fields.node_range.as_ref().map(BankEntryRange::from),
            child_count: fields.child_count.as_ref().map(BankField::from),
            checked_children: fields.children.as_ref().map(|_| fields.checked_children),
            checked_children_range: fields.children.as_ref().map(BankEntryRange::from),
            trailing_range: fields.trailing.as_ref().map(BankEntryRange::from),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActorMixerChildRecord {
    pub(super) ordinal: DecimalU64,
    pub(super) child_id: BankField<u32>,
}
