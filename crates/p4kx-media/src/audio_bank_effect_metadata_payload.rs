// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_property_payload::{NodePropertyListRecord, NodePropertyRecord};
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const PLUGIN: &str = "audio-bank-hirc-plugin";
pub(crate) const BANK_DATA: &str = "audio-bank-hirc-plugin-bank-data";
pub(crate) const OVERRIDE: &str = "audio-bank-hirc-plugin-state-override";
pub(crate) const MODULATOR: &str = "audio-bank-hirc-modulator";
pub(crate) const MODULATOR_PROPERTY: &str = "audio-bank-hirc-modulator-property";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum MetadataPayload<'row> {
    #[serde(rename = "bank-hirc-plugin")]
    Plugin(&'row NodeDeclaration<ObservedPlugin>),
    #[serde(rename = "bank-hirc-plugin-bank-data")]
    BankData(PluginBankData),
    #[serde(rename = "bank-hirc-plugin-state-override")]
    StateOverride(PluginStateOverride),
    #[serde(rename = "bank-hirc-modulator")]
    Modulator(&'row NodeDeclaration<ObservedModulator>),
    #[serde(rename = "bank-hirc-modulator-property")]
    ModulatorProperty(&'row NodePropertyRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetadataCoverage {
    pub(crate) object_kind: &'static str,
    pub(crate) checked_range: BankEntryRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) unprojected_range: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservedPlugin {
    pub(crate) coverage: MetadataCoverage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) plugin_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parameters: Option<PluginParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bank_data_count: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) override_count: Option<BankField<u16>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PluginParameters {
    pub(crate) declared_size: BankField<u32>,
    pub(crate) range: BankEntryRange,
    pub(crate) complete: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PluginBankData {
    pub(crate) index: BankField<u8>,
    pub(crate) source_id: BankField<u32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PluginStateOverride {
    pub(crate) property_id: BankField<u32>,
    pub(crate) accumulation: BankField<u8>,
    pub(crate) value_bits: BankField<u32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservedModulator {
    pub(crate) coverage: MetadataCoverage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) properties: Option<NodePropertyListRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ranged_modifiers: Option<NodePropertyListRecord>,
}
