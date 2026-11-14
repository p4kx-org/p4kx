// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::BankEventCountEncoding;
use schemars::JsonSchema;
use serde::Serialize;

use crate::MediaError;
use crate::audio_bank_payload::BankEntryRange;

use super::reference::{CandidateRecord, ResolutionRecord};

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum EventPayload {
    #[serde(rename = "bank-hirc-event")]
    Event(EventRecord),
    #[serde(rename = "bank-hirc-event-action")]
    Action(EventActionRecord),
    #[serde(rename = "bank-hirc-event-action-resolution")]
    Resolution(ResolutionRecord),
    #[serde(rename = "bank-hirc-event-action-candidate")]
    Candidate(CandidateRecord),
    #[serde(rename = "bank-hirc-event-tail")]
    Tail(EventTail),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum EventRecord {
    Decoded {
        action_count: u32,
        count_encoding: CountEncoding,
        count_range: BankEntryRange,
    },
    Malformed {
        action_count: Option<u32>,
        count_encoding: CountEncoding,
        count_range: BankEntryRange,
    },
    Unsupported {
        action_count: Option<()>,
        count_encoding: Option<()>,
        count_range: Option<()>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CountEncoding {
    FixedU32,
    VariableLength,
}

impl TryFrom<BankEventCountEncoding> for CountEncoding {
    type Error = MediaError;

    fn try_from(value: BankEventCountEncoding) -> Result<Self, Self::Error> {
        match value {
            BankEventCountEncoding::FixedU32 => Ok(Self::FixedU32),
            BankEventCountEncoding::VariableLength => Ok(Self::VariableLength),
            _ => Err(MediaError::Contract(
                "unexpected HIRC Event count encoding".into(),
            )),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventActionRecord {
    pub(crate) ordinal: DecimalU64,
    pub(crate) action_id: u32,
    pub(crate) id_range: BankEntryRange,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EventTail {
    pub(crate) trailing_range: BankEntryRange,
}
