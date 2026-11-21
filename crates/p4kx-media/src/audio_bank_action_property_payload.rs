// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_property_payload::{NodePropertyListRecord, NodePropertyRecord};
use crate::audio_bank_payload::BankField;

pub(crate) const SUMMARY_NAMESPACE: &str = "audio-bank-hirc-action-properties";
pub(crate) const PROPERTY_NAMESPACE: &str = "audio-bank-hirc-action-property";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum ActionPropertyPayload<'row> {
    #[serde(rename = "bank-hirc-action-properties")]
    Summary(&'row NodeDeclaration<ActionPropertyObservations>),
    #[serde(rename = "bank-hirc-action-property")]
    Property(&'row NodePropertyRecord),
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActionPropertyObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fade_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) properties: Option<NodePropertyListRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ranged_modifiers: Option<NodePropertyListRecord>,
}
