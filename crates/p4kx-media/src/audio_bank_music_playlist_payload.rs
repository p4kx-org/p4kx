// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{
    BankHircField, BankMusicPlaylistFields, BankMusicPlaylistItem, MusicPlayMode,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_music_value::{MusicFloat, MusicNodeValues, MusicStingerRecord, entry_range};
use crate::audio_bank_payload::BankEntryRange;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum MusicPlaylistPayload {
    #[serde(rename = "bank-music-playlist")]
    Playlist(PlaylistRecord),
    #[serde(rename = "bank-music-playlist-child")]
    Child(PlaylistChildRecord),
    #[serde(rename = "bank-music-playlist-stinger")]
    Stinger(MusicStingerRecord),
    #[serde(rename = "bank-music-playlist-item")]
    Item(PlaylistItemRecord),
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PlaylistLayout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlaylistRecord {
    layout: PlaylistLayout,
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
    item_count: Option<u32>,
}

impl PlaylistRecord {
    pub(super) fn new(fields: &BankMusicPlaylistFields, layout: PlaylistLayout) -> Self {
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
            item_count: fields.item_count.as_ref().map(|word| word.value),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlaylistChildRecord {
    ordinal: DecimalU64,
    child_id: u32,
    id_range: BankEntryRange,
}

impl PlaylistChildRecord {
    pub(super) const fn new(ordinal: u64, child: &BankHircField<u32>) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            child_id: child.value,
            id_range: entry_range(&child.range),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlaylistItemRecord {
    ordinal: DecimalU64,
    segment_id: u32,
    item_id: u32,
    children: u32,
    play_mode_code: u32,
    play_mode: PlaylistMode,
    loops: i16,
    loop_min: i16,
    loop_max: i16,
    weight: u32,
    avoid_repeat_count: u16,
    uses_weight_code: u8,
    uses_weight: bool,
    shuffle_code: u8,
    shuffle: bool,
    item_range: BankEntryRange,
}

impl PlaylistItemRecord {
    pub(super) fn new(ordinal: u64, item: &BankMusicPlaylistItem) -> Self {
        let value = item.value();
        Self {
            ordinal: DecimalU64::new(ordinal),
            segment_id: value.segment_id,
            item_id: value.item_id,
            children: value.children,
            play_mode_code: value.play_mode,
            play_mode: PlaylistMode::from(value.mode()),
            loops: value.loops,
            loop_min: value.loop_min,
            loop_max: value.loop_max,
            weight: value.weight,
            avoid_repeat_count: value.avoid_repeat_count,
            uses_weight_code: item.uses_weight_code.value,
            uses_weight: value.uses_weight,
            shuffle_code: item.shuffle_code.value,
            shuffle: value.shuffle,
            item_range: entry_range(&item.range),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum PlaylistMode {
    Random,
    SequenceContinuous,
    RandomStep,
    SequenceStep,
    Other,
}

impl From<MusicPlayMode> for PlaylistMode {
    fn from(mode: MusicPlayMode) -> Self {
        match mode {
            MusicPlayMode::Random => Self::Random,
            MusicPlayMode::SequenceContinuous => Self::SequenceContinuous,
            MusicPlayMode::RandomStep => Self::RandomStep,
            MusicPlayMode::SequenceStep => Self::SequenceStep,
            MusicPlayMode::Other(_) => Self::Other,
        }
    }
}
