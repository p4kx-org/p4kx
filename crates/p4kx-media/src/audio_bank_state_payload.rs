// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_payload::{BankEntryRange, BankField};

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum StatePayload {
    #[serde(rename = "bank-hirc-state")]
    Standalone(StandaloneStateRecord),
    #[serde(rename = "bank-hirc-state-property")]
    Property(StatePropertyRecord),
    #[serde(rename = "bank-hirc-inline-state")]
    Inline(InlineStateRecord),
    #[serde(rename = "bank-hirc-inline-state-binding-count")]
    InlineBindingCount(StateCountRecord),
    #[serde(rename = "bank-hirc-inline-state-binding")]
    InlineBinding(StateBindingRecord),
    #[serde(rename = "bank-hirc-inline-state-group-count")]
    InlineGroupCount(StateCountRecord),
    #[serde(rename = "bank-hirc-inline-state-group")]
    InlineGroup(StateGroupRecord),
    #[serde(rename = "bank-hirc-inline-state-entry")]
    InlineEntry(StateEntryRecord),
    #[serde(rename = "bank-hirc-inline-state-property")]
    InlineProperty(StatePropertyRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum StandaloneStateRecord {
    Decoded {
        version: u32,
        property_count: BankField<u16>,
        consumed_range: BankEntryRange,
        trailing_range: BankEntryRange,
    },
    Unsupported {
        version: u32,
    },
    Malformed {
        version: u32,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum InlineStateRecord {
    Decoded {
        version: u32,
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
pub(crate) struct StateCountRecord {
    pub(crate) count: BankField<u32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateBindingRecord {
    pub(crate) property_id: BankField<u32>,
    pub(crate) accumulation: BankField<u8>,
    pub(crate) in_db: BankField<u8>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateGroupRecord {
    pub(crate) group_id: BankField<u32>,
    pub(crate) synchronization: BankField<u8>,
    pub(crate) state_count: BankField<u32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct StateEntryRecord {
    pub(crate) state_id: BankField<u32>,
    pub(crate) property_count: BankField<u16>,
    pub(crate) consumed_range: BankEntryRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatePropertyRecord {
    pub(crate) property_id: BankField<u16>,
    pub(crate) value_bits: BankField<u32>,
    pub(crate) value: Option<f32>,
}
