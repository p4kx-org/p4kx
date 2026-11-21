// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_payload::BankField;

pub(crate) const NAMESPACE: &str = "audio-bank-music-stinger-schedule";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum MusicStingerSchedulePayload<'row> {
    #[serde(rename = "bank-music-stinger-schedule")]
    Schedule(&'row MusicStingerScheduleRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum MusicStingerScheduleRecord {
    Decoded {
        ordinal: DecimalU64,
        trigger_id: BankField<u32>,
        segment_id: BankField<u32>,
        sync_play_at: BankField<u32>,
        cue_filter_hash: BankField<u32>,
        dont_repeat_time: StingerRepeatTime,
        segment_look_ahead: BankField<u32>,
    },
    Malformed {
        ordinal: DecimalU64,
        trigger_id: BankField<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        segment_id: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sync_play_at: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cue_filter_hash: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dont_repeat_time: Option<StingerRepeatTime>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct StingerRepeatTime {
    pub(crate) bits: BankField<u32>,
    pub(crate) signed_value: i32,
}
