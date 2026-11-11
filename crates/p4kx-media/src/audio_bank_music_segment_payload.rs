// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::BankMusicSegmentFields;
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_music_value::{MusicFloat, MusicNodeValues, MusicStingerRecord};
use crate::audio_bank_payload::BankEntryRange;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum MusicSegmentPayload<'a> {
    #[serde(rename = "bank-music-segment")]
    Segment(SegmentRecord),
    #[serde(rename = "bank-music-segment-child")]
    Child(MusicChildRecord),
    #[serde(rename = "bank-music-segment-stinger")]
    Stinger(MusicStingerRecord),
    #[serde(rename = "bank-music-segment-marker")]
    Marker(MusicMarkerRecord<'a>),
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MusicSegmentLayout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SegmentRecord {
    layout: MusicSegmentLayout,
    parent_id: Option<u32>,
    child_count: Option<u32>,
    grid_period: Option<MusicFloat>,
    grid_offset: Option<MusicFloat>,
    tempo: Option<MusicFloat>,
    beats_per_bar: Option<u8>,
    beat_value: Option<u8>,
    stinger_count: Option<u32>,
    duration: Option<MusicFloat>,
    marker_count: Option<u32>,
}

impl SegmentRecord {
    pub(super) fn read(fields: &BankMusicSegmentFields, layout: MusicSegmentLayout) -> Self {
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
            duration: fields
                .duration
                .as_ref()
                .map(|word| MusicFloat::from_f64(word.value)),
            marker_count: fields.marker_count.as_ref().map(|word| word.value),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MusicChildRecord {
    pub(super) ordinal: DecimalU64,
    pub(super) track_id: u32,
    pub(super) id_range: BankEntryRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MusicMarkerRecord<'a> {
    pub(super) ordinal: DecimalU64,
    pub(super) marker_id: u32,
    position: MusicFloat,
    pub(super) name: &'a str,
    pub(super) encoding: MusicMarkerEncoding,
    pub(super) name_range: BankEntryRange,
    pub(super) terminator_offset: DecimalU64,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum MusicMarkerEncoding {
    Utf8,
    InvalidUtf8,
}

impl<'a> MusicMarkerRecord<'a> {
    pub(super) fn new(
        ordinal: u64,
        marker: &p4kx_extraction::bank::BankMusicMarker,
        name: &'a str,
        encoding: MusicMarkerEncoding,
    ) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            marker_id: marker.id.value,
            position: MusicFloat::from_f64(marker.position.value),
            name,
            encoding,
            name_range: entry_range(&marker.name_range),
            terminator_offset: DecimalU64::new(marker.terminator),
        }
    }
}

pub(super) const fn entry_range(range: &Range<u64>) -> BankEntryRange {
    BankEntryRange::new(range.start, range.end)
}
