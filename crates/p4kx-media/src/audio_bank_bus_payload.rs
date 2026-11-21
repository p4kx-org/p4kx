// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const SUMMARY: &str = "audio-bank-hirc-bus";
pub(crate) const DUCK: &str = "audio-bank-hirc-bus-duck";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum BusPayload<'row> {
    #[serde(rename = "bank-hirc-bus")]
    Bus(&'row NodeDeclaration<BusObservations>),
    #[serde(rename = "bank-hirc-bus-duck")]
    Duck(&'row DuckRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BusObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) override_bus_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) device_shareset_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) playback_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_instances: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) channel_config_bits: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) hdr_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) recovery_time: Option<BankField<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_duck_volume_bits: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) duck_count: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) raw_remainder: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DuckRecord {
    pub(crate) bus_id: BankField<u32>,
    pub(crate) volume_bits: BankField<u32>,
    pub(crate) fade_out_time: BankField<i32>,
    pub(crate) fade_in_time: BankField<i32>,
    pub(crate) fade_curve: BankField<u8>,
    pub(crate) target_property: BankField<u8>,
}
