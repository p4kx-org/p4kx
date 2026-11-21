// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_payload::BankEntryRange;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum BankStidPayload<'source> {
    #[serde(rename = "bank-stid")]
    Stid(BankStidHeader),
    #[serde(rename = "bank-name")]
    Name(BankNameRecord<'source>),
    #[serde(rename = "bank-stid-tail")]
    Tail(BankStidTail),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankStidHeader {
    pub(crate) section_ordinal: DecimalU64,
    pub(crate) string_type: u32,
    pub(crate) declared_names: u32,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankNameRecord<'source> {
    pub(crate) section_ordinal: DecimalU64,
    pub(crate) ordinal: DecimalU64,
    pub(crate) bank_id: u32,
    pub(crate) name_length: u8,
    pub(crate) header_range: BankEntryRange,
    pub(crate) name_range: BankEntryRange,
    #[schemars(length(max = 255))]
    pub(crate) name_bytes: &'source [u8],
    pub(crate) name_text: BankNameText<'source>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "encoding", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum BankNameText<'source> {
    Utf8 {
        text: &'source str,
    },
    InvalidUtf8 {
        valid_up_to: u8,
        error_length: Option<u8>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankStidTail {
    pub(crate) section_ordinal: DecimalU64,
    pub(crate) trailing_range: BankEntryRange,
}
