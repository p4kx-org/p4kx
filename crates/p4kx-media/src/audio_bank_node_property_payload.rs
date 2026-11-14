// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_property_rows::PropertyInterpretation;
use crate::audio_bank_payload::{BankEntryRange, BankField};

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum NodePropertyPayload<'row> {
    BankHircNodeProperties(&'row NodePropertySummary),
    BankHircNodeProperty(&'row NodePropertyRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum NodePropertySummary {
    Decoded {
        version: u32,
        observed: NodePropertyObservations,
    },
    Malformed {
        version: u32,
        observed: NodePropertyObservations,
    },
    Absent {
        version: u32,
    },
    Unsupported {
        version: u32,
    },
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodePropertyObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) override_bus_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) parent_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) priority_flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) properties: Option<NodePropertyListRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ranged_modifiers: Option<NodePropertyListRecord>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodePropertyListRecord {
    pub(crate) declared_count: BankField<u8>,
    pub(crate) checked_id_count: u8,
    pub(crate) checked_word_count: u16,
    pub(crate) ids_range: BankEntryRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) words_range: Option<BankEntryRange>,
    pub(crate) complete: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodePropertyRecord {
    pub(crate) ordinal: u8,
    pub(crate) property_id: BankField<u8>,
    pub(crate) words: NodePropertyWords,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) interpretation: Option<PropertyInterpretation>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum NodePropertyWords {
    Scalar {
        #[serde(skip_serializing_if = "Option::is_none")]
        scalar_bits: Option<BankField<u32>>,
    },
    Ranged {
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum_bits: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum_bits: Option<BankField<u32>>,
    },
}
