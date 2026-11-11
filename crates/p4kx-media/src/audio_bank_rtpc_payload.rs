// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_payload::{BankEntryRange, BankField};

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum RtpcPayload<'row> {
    #[serde(rename = "bank-hirc-sound-rtpc")]
    Summary(&'row RtpcRecord),
    #[serde(rename = "bank-hirc-sound-rtpc-curve")]
    Curve(&'row RtpcCurveRecord),
    #[serde(rename = "bank-hirc-sound-rtpc-point")]
    Point(&'row RtpcPointRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum RtpcRecord {
    Decoded {
        version: u32,
        curve_count: BankField<u16>,
        checked_curves: u16,
        checked_range: BankEntryRange,
    },
    Malformed {
        version: u32,
        curve_count: BankField<u16>,
        checked_curves: u16,
        checked_range: BankEntryRange,
    },
    Absent {
        version: u32,
    },
    Unavailable {
        version: u32,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RtpcCurveRecord {
    pub(crate) rtpc_id: BankField<u32>,
    pub(crate) rtpc_type: BankField<u8>,
    pub(crate) accumulation: BankField<u8>,
    pub(crate) parameter_id: BankField<u32>,
    pub(crate) curve_id: BankField<u32>,
    pub(crate) scaling: BankField<u8>,
    pub(crate) point_count: BankField<u16>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RtpcPointRecord {
    pub(crate) from_bits: BankField<u32>,
    pub(crate) to_bits: BankField<u32>,
    pub(crate) interpolation: BankField<u32>,
    pub(crate) input: Option<f32>,
    pub(crate) output: Option<f32>,
}
