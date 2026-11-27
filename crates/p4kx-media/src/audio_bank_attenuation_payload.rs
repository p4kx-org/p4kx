// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_payload::{BankEntryRange, BankField};
use crate::audio_bank_rtpc_payload::RtpcPointRecord;

pub(crate) const SUMMARY: &str = "audio-bank-hirc-attenuation";
pub(crate) const SELECTOR: &str = "audio-bank-hirc-attenuation-selector";
pub(crate) const CURVE: &str = "audio-bank-hirc-attenuation-curve";
pub(crate) const POINT: &str = "audio-bank-hirc-attenuation-point";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum AttenuationPayload<'row> {
    #[serde(rename = "bank-hirc-attenuation")]
    Summary(&'row NodeDeclaration<AttenuationObservations>),
    #[serde(rename = "bank-hirc-attenuation-selector")]
    Selector(AttenuationSelector),
    #[serde(rename = "bank-hirc-attenuation-curve")]
    Curve(&'row AttenuationCurve),
    #[serde(rename = "bank-hirc-attenuation-point")]
    Point(&'row RtpcPointRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttenuationObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) height_spread_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cone_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cone_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cone: Option<AttenuationCone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) curve_count: Option<BankField<u8>>,
    pub(crate) checked_range: BankEntryRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) unprojected_range: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttenuationCone {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inside_degrees_bits")]
    pub(crate) inside_degrees: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outside_degrees_bits")]
    pub(crate) outside_degrees: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outside_volume_bits")]
    pub(crate) outside_volume: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "low_pass_bits")]
    pub(crate) low_pass: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "high_pass_bits")]
    pub(crate) high_pass: Option<BankField<u32>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttenuationSelector {
    pub(crate) curve_index: BankField<i8>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttenuationCurve {
    pub(crate) scaling: BankField<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) point_count: Option<BankField<u16>>,
    pub(crate) points_range: BankEntryRange,
}
