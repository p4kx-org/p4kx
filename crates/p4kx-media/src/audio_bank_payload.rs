// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::OffsetBasis;
use crate::audio_bank_failure::BankFailureRecord;

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum BankPayload {
    Bank(BankHeaderRecord),
    BankSection(BankSectionRecord),
    BankMedia(BankMediaRecord),
    BankObject(BankObjectRecord),
    BankHirc(BankHircRecord),
    BankFailure(BankFailureRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankHeaderRecord {
    pub(crate) section_ordinal: DecimalU64,
    pub(crate) version: u32,
    pub(crate) version_supported: bool,
    pub(crate) bank_id: u32,
    pub(crate) language_id: u32,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankSectionRecord {
    pub(crate) ordinal: DecimalU64,
    /// Four original bytes, including non-UTF-8 tags.
    pub(crate) tag: [u8; 4],
    pub(crate) header_range: BankEntryRange,
    pub(crate) body_range: BankEntryRange,
    /// Another occurrence of a section whose format admits one.
    pub(crate) duplicate_unique: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankMediaRecord {
    pub(crate) section_ordinal: DecimalU64,
    pub(crate) ordinal: DecimalU64,
    pub(crate) media_id: u32,
    /// Original offset measured from the start of the DATA body.
    pub(crate) data_offset: u32,
    pub(crate) data_length: u32,
    pub(crate) index_range: BankEntryRange,
    pub(crate) data: BankDataLink,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum BankDataLink {
    Resolved {
        section_ordinal: DecimalU64,
        range: BankEntryRange,
        class: &'static str,
        declared_wem_length: Option<DecimalU64>,
        prefetch: bool,
    },
    Missing,
    Ambiguous {
        candidates: DecimalU64,
    },
    Unavailable,
    OutsideData {
        section_ordinal: DecimalU64,
        data_length: DecimalU64,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankObjectRecord {
    pub(crate) section_ordinal: DecimalU64,
    pub(crate) ordinal: DecimalU64,
    pub(crate) type_code: u8,
    /// Interpretation under the uniquely observed bank version, when available.
    pub(crate) kind: Option<&'static str>,
    pub(crate) object_id: u32,
    pub(crate) header_range: BankEntryRange,
    /// Original object body, including its ID.
    pub(crate) body_range: BankEntryRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankHircRecord {
    pub(crate) section_ordinal: DecimalU64,
    pub(crate) declared_objects: u32,
    pub(crate) trailing_range: BankEntryRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BankEntryRange {
    #[schemars(extend("const" = serde_json::json!({"type":"entry"})))]
    pub(crate) basis: OffsetBasis,
    pub(crate) start: DecimalU64,
    pub(crate) end_exclusive: DecimalU64,
}

impl BankEntryRange {
    pub(crate) const fn new(start: u64, end_exclusive: u64) -> Self {
        Self {
            basis: OffsetBasis::Entry,
            start: DecimalU64::new(start),
            end_exclusive: DecimalU64::new(end_exclusive),
        }
    }
}
