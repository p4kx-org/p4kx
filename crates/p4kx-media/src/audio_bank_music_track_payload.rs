// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{
    BankHircField, BankMusicTrackClip, BankMusicTrackFields, BankMusicTrackSource,
    BankMusicTrackSwitchFields, MusicTrackKind,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::MediaError;
use crate::audio_bank_music_value::MusicFloat;
use crate::audio_bank_payload::BankEntryRange;
use crate::audio_bank_sound_payload::Storage;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum MusicTrackPayload {
    #[serde(rename = "bank-music-track")]
    Track(TrackRecord),
    #[serde(rename = "bank-music-track-source")]
    Source(TrackSourceRecord),
    #[serde(rename = "bank-music-track-clip")]
    Clip(TrackClipRecord),
    #[serde(rename = "bank-music-track-switch-member")]
    SwitchMember(TrackSwitchMember),
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TrackLayout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackRecord {
    layout: TrackLayout,
    source_count: Option<u32>,
    clip_count: Option<u32>,
    automation_count: Option<u32>,
    automation_range: Option<BankEntryRange>,
    parent_id: Option<u32>,
    kind_code: Option<u8>,
    kind: Option<TrackKind>,
    switch: Option<TrackSwitch>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum TrackKind {
    Normal,
    Random,
    Sequence,
    Switch,
    Other,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TrackSwitch {
    group_type: Option<u8>,
    group_id: Option<u32>,
    default_switch: Option<u32>,
    member_count: Option<u32>,
    transition_range: Option<BankEntryRange>,
}

impl TrackRecord {
    pub(super) fn new(fields: &BankMusicTrackFields, layout: TrackLayout) -> Self {
        Self {
            layout,
            source_count: word(fields.source_count.as_ref()),
            clip_count: word(fields.clip_count.as_ref()),
            automation_count: word(fields.automation_count.as_ref()),
            automation_range: fields.automation_range.as_ref().map(entry_range),
            parent_id: word(fields.parent.parent_id.as_ref()),
            kind_code: word(fields.kind_code.as_ref()),
            kind: fields.kind().map(TrackKind::from),
            switch: fields.switch.as_ref().map(TrackSwitch::new),
        }
    }
}

impl From<MusicTrackKind> for TrackKind {
    fn from(kind: MusicTrackKind) -> Self {
        match kind {
            MusicTrackKind::Normal => Self::Normal,
            MusicTrackKind::Random => Self::Random,
            MusicTrackKind::Sequence => Self::Sequence,
            MusicTrackKind::Switch => Self::Switch,
            MusicTrackKind::Other(_) => Self::Other,
        }
    }
}

impl TrackSwitch {
    fn new(fields: &BankMusicTrackSwitchFields) -> Self {
        Self {
            group_type: word(fields.group_type.as_ref()),
            group_id: word(fields.group_id.as_ref()),
            default_switch: word(fields.default_switch.as_ref()),
            member_count: word(fields.member_count.as_ref()),
            transition_range: fields.transition_range.as_ref().map(entry_range),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Independent Wwise flags are separate fields"
)]
pub(crate) struct TrackSourceRecord {
    ordinal: DecimalU64,
    plugin_id: u32,
    storage_code: u8,
    storage: Storage,
    source_id: u32,
    cache_id: Option<u32>,
    in_memory_size: u32,
    source_bits: u8,
    language_specific: bool,
    source_plugin: bool,
    external_source: bool,
    midi: bool,
    has_media_file: bool,
    plugin_parameter_length: Option<u32>,
    plugin_parameters_range: Option<BankEntryRange>,
    source_range: BankEntryRange,
}

impl TrackSourceRecord {
    pub(super) fn new(ordinal: u64, source: &BankMusicTrackSource) -> Result<Self, MediaError> {
        let value = source.value;
        let storage_code = word(source.fields.storage_code.as_ref())
            .ok_or_else(|| MediaError::Contract("MusicTrack source has no storage byte".into()))?;
        Ok(Self {
            ordinal: DecimalU64::new(ordinal),
            plugin_id: value.plugin_id,
            storage_code,
            storage: Storage::from(value.storage),
            source_id: value.source_id,
            cache_id: word(source.fields.cache_id.as_ref()),
            in_memory_size: value.in_memory_size,
            source_bits: value.source_bits,
            language_specific: value.is_language_specific(),
            source_plugin: value.is_source_plugin(),
            external_source: value.is_external_source(),
            midi: value.is_midi(),
            has_media_file: value.has_media_file(),
            plugin_parameter_length: word(source.plugin_parameter_length.as_ref()),
            plugin_parameters_range: source.plugin_parameters.as_ref().map(entry_range),
            source_range: entry_range(&source.range),
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackClipRecord {
    ordinal: DecimalU64,
    source_id: u32,
    cache_id: Option<u32>,
    event_id: u32,
    play_at: MusicFloat,
    begin_trim: MusicFloat,
    end_trim: MusicFloat,
    source_duration: MusicFloat,
    clip_range: BankEntryRange,
}

impl TrackClipRecord {
    pub(super) fn new(ordinal: u64, clip: &BankMusicTrackClip) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            source_id: clip.source_id.value,
            cache_id: word(clip.cache_id.as_ref()),
            event_id: clip.event_id.value,
            play_at: MusicFloat::from_f64(clip.play_at.value),
            begin_trim: MusicFloat::from_f64(clip.begin_trim.value),
            end_trim: MusicFloat::from_f64(clip.end_trim.value),
            source_duration: MusicFloat::from_f64(clip.source_duration.value),
            clip_range: entry_range(&clip.range),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TrackSwitchMember {
    ordinal: DecimalU64,
    value_id: u32,
    value_id_range: BankEntryRange,
}

impl TrackSwitchMember {
    pub(super) const fn new(ordinal: u64, value: &BankHircField<u32>) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            value_id: value.value,
            value_id_range: entry_range(&value.range),
        }
    }
}

pub(super) const fn word<T: Copy>(field: Option<&BankHircField<T>>) -> Option<T> {
    match field {
        Some(field) => Some(field.value),
        None => None,
    }
}

const fn entry_range(range: &Range<u64>) -> BankEntryRange {
    BankEntryRange::new(range.start, range.end)
}
