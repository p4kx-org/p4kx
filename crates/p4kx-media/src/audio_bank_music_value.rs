// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{BankHircField, BankMusicNodeFields, BankMusicStinger};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_payload::BankEntryRange;

pub(crate) struct MusicNodeValues {
    pub(crate) parent_id: Option<u32>,
    pub(crate) child_count: Option<u32>,
    pub(crate) grid_period: Option<MusicFloat>,
    pub(crate) grid_offset: Option<MusicFloat>,
    pub(crate) tempo: Option<MusicFloat>,
    pub(crate) beats_per_bar: Option<u8>,
    pub(crate) beat_value: Option<u8>,
    pub(crate) stinger_count: Option<u32>,
}

impl MusicNodeValues {
    pub(crate) fn new(node: &BankMusicNodeFields) -> Self {
        Self {
            parent_id: node.parent.parent_id.as_ref().map(|word| word.value),
            child_count: node.child_count.as_ref().map(|word| word.value),
            grid_period: node
                .grid_period
                .as_ref()
                .map(|word| MusicFloat::from_f64(word.value)),
            grid_offset: node
                .grid_offset
                .as_ref()
                .map(|word| MusicFloat::from_f64(word.value)),
            tempo: node
                .tempo
                .as_ref()
                .map(|word| MusicFloat::from_f32(word.value)),
            beats_per_bar: node.beats_per_bar.as_ref().map(|word| word.value),
            beat_value: node.beat_value.as_ref().map(|word| word.value),
            stinger_count: node.stinger_count.as_ref().map(|word| word.value),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MusicStingerRecord {
    ordinal: DecimalU64,
    trigger_id: u32,
    segment_id: u32,
    opaque_range: BankEntryRange,
}

impl MusicStingerRecord {
    pub(crate) const fn new(ordinal: u64, value: &BankMusicStinger) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            trigger_id: value.trigger_id.value,
            segment_id: value.segment_id.value,
            opaque_range: entry_range(&value.opaque_range),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MusicNodeChildRecord {
    ordinal: DecimalU64,
    child_id: u32,
    id_range: BankEntryRange,
}

impl MusicNodeChildRecord {
    pub(crate) const fn new(ordinal: u64, child: &BankHircField<u32>) -> Self {
        Self {
            ordinal: DecimalU64::new(ordinal),
            child_id: child.value,
            id_range: entry_range(&child.range),
        }
    }
}

pub(crate) const fn entry_range(range: &Range<u64>) -> BankEntryRange {
    BankEntryRange::new(range.start, range.end)
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MusicFloat {
    raw_bits: String,
    finite: Option<f64>,
}

impl MusicFloat {
    pub(crate) fn from_f32(bits: u32) -> Self {
        let number = f32::from_bits(bits);
        Self {
            raw_bits: format!("{bits:08x}"),
            finite: number.is_finite().then_some(f64::from(number)),
        }
    }

    pub(crate) fn from_f64(bits: u64) -> Self {
        let number = f64::from_bits(bits);
        Self {
            raw_bits: format!("{bits:016x}"),
            finite: number.is_finite().then_some(number),
        }
    }
}
