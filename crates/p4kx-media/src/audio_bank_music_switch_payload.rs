// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{
    BankMusicSwitchFields, BankMusicSwitchGroup, BankMusicSwitchNode, BankMusicSwitchTarget,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_music_value::{
    MusicFloat, MusicNodeChildRecord, MusicNodeValues, MusicStingerRecord, entry_range,
};
use crate::audio_bank_payload::BankEntryRange;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum MusicSwitchPayload {
    #[serde(rename = "bank-music-switch")]
    Switch(Box<SwitchRecord>),
    #[serde(rename = "bank-music-switch-child")]
    Child(MusicNodeChildRecord),
    #[serde(rename = "bank-music-switch-stinger")]
    Stinger(MusicStingerRecord),
    #[serde(rename = "bank-music-switch-group")]
    Group(SwitchGroupRecord),
    #[serde(rename = "bank-music-switch-node")]
    Node(SwitchNodeRecord),
    #[serde(rename = "bank-music-switch-target")]
    Target(SwitchTargetRecord),
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SwitchLayout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchRecord {
    layout: SwitchLayout,
    parent_id: Option<u32>,
    child_count: Option<u32>,
    grid_period: Option<MusicFloat>,
    grid_offset: Option<MusicFloat>,
    tempo: Option<MusicFloat>,
    beats_per_bar: Option<u8>,
    beat_value: Option<u8>,
    stinger_count: Option<u32>,
    transition_rule_count: Option<u32>,
    transition_rules_range: Option<BankEntryRange>,
    continue_playback_code: Option<u8>,
    group_count: Option<u32>,
    tree_size: Option<u32>,
    tree_mode_code: Option<u8>,
    tree_range: Option<BankEntryRange>,
}

impl SwitchRecord {
    pub(super) fn new(fields: &BankMusicSwitchFields, layout: SwitchLayout) -> Self {
        let MusicNodeValues {
            parent_id,
            child_count,
            grid_period,
            grid_offset,
            tempo,
            beats_per_bar,
            beat_value,
            stinger_count,
        } = MusicNodeValues::new(&fields.node);
        Self {
            layout,
            parent_id,
            child_count,
            grid_period,
            grid_offset,
            tempo,
            beats_per_bar,
            beat_value,
            stinger_count,
            transition_rule_count: fields
                .transitions
                .rule_count
                .as_ref()
                .map(|word| word.value),
            transition_rules_range: fields.transitions.rules_range.as_ref().map(entry_range),
            continue_playback_code: fields
                .continue_playback_code
                .as_ref()
                .map(|word| word.value),
            group_count: fields.group_count.as_ref().map(|word| word.value),
            tree_size: fields.tree_size.as_ref().map(|word| word.value),
            tree_mode_code: fields.tree_mode_code.as_ref().map(|word| word.value),
            tree_range: fields.tree_range.as_ref().map(entry_range),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchGroupRecord {
    ordinal: DecimalU64,
    group_id: u32,
    type_code: Option<u8>,
}

impl SwitchGroupRecord {
    pub(super) fn new(ordinal: u64, group: &BankMusicSwitchGroup) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            group_id: group.id.value,
            type_code: group.type_code.as_ref().map(|word| word.value),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchNodeRecord {
    ordinal: DecimalU64,
    key: u32,
    link_word: u32,
    weight: u16,
    probability: u16,
    node_range: BankEntryRange,
}

impl SwitchNodeRecord {
    pub(super) const fn new(ordinal: u64, node: &BankMusicSwitchNode) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            key: node.key.value,
            link_word: node.link.value,
            weight: node.weight.value,
            probability: node.probability.value,
            node_range: entry_range(&node.range),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SwitchTargetRecord {
    ordinal: DecimalU64,
    node_index: u32,
    level: u32,
    target_id: u32,
    ancestor_nodes: Vec<u32>,
}

impl SwitchTargetRecord {
    pub(super) const fn new(
        ordinal: u64,
        target: &BankMusicSwitchTarget,
        ancestor_nodes: Vec<u32>,
    ) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            node_index: target.node_index,
            level: target.level,
            target_id: target.id.value,
            ancestor_nodes,
        }
    }
}
