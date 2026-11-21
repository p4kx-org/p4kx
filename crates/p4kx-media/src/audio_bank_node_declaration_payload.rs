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
pub(crate) enum NodeDeclarationPayload<'row> {
    #[serde(rename = "bank-hirc-node-fx")]
    Fx(&'row NodeDeclaration<FxObservations>),
    #[serde(rename = "bank-hirc-node-fx-slot")]
    Slot(&'row FxSlotRecord),
    #[serde(rename = "bank-hirc-node-aux")]
    Aux(&'row NodeDeclaration<AuxObservations>),
}

#[derive(Serialize, JsonSchema)]
#[schemars(rename = "NodeDeclaration_{T}")]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum NodeDeclaration<T> {
    Decoded { version: u32, observed: T },
    Malformed { version: u32, observed: T },
    Absent { version: u32 },
    Unavailable { version: u32 },
    Unsupported { version: u32 },
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FxObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) override_parent_bits: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) declared_count: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bypass_all_bits: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) slots_range: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NodeReferenceStatus {
    Unchecked,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum FxSlotRecord {
    Decoded {
        ordinal: u8,
        slot_index: BankField<u8>,
        effect_id: BankField<u32>,
        slot_bits: BankField<u8>,
        reference_resolution: NodeReferenceStatus,
    },
    Malformed {
        ordinal: u8,
        slot_index: BankField<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        effect_id: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reference_resolution: Option<NodeReferenceStatus>,
    },
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuxObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) flags: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) user_bus_ids: Option<[Option<BankField<u32>>; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reflections_bus_id: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reference_resolution: Option<NodeReferenceStatus>,
}
