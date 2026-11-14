// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::{NodeDeclaration, NodeReferenceStatus};
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const SUMMARY_NAMESPACE: &str = "audio-bank-hirc-blend-container";
pub(crate) const LAYER_NAMESPACE: &str = "audio-bank-hirc-blend-layer";
pub(crate) const ASSOCIATION_NAMESPACE: &str = "audio-bank-hirc-blend-association";
pub(crate) const POINT_NAMESPACE: &str = "audio-bank-hirc-blend-point";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum BlendPayload<'row> {
    #[serde(rename = "bank-hirc-blend-container")]
    Summary(&'row NodeDeclaration<BlendObservations>),
    #[serde(rename = "bank-hirc-blend-layer")]
    Layer(&'row BlendLayerRecord),
    #[serde(rename = "bank-hirc-blend-association")]
    Association(&'row BlendAssociationRecord),
    #[serde(rename = "bank-hirc-blend-point")]
    Point(&'row BlendPointRecord),
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlendObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) layers: Option<BlendTable>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) continuous_validation: Option<BankField<u8>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlendTable {
    pub(crate) declared_count: BankField<u32>,
    pub(crate) checked_range: BankEntryRange,
    pub(crate) coverage: BlendTableCoverage,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum BlendTableCoverage {
    Complete,
    Truncated,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum BlendLayerRecord {
    Decoded {
        layer_id: BankField<u32>,
        crossfade_rtpc_id: BankField<u32>,
        crossfade_rtpc_type: BankField<u8>,
        associations: BlendTable,
    },
    Malformed {
        layer_id: BankField<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        crossfade_rtpc_id: Option<BankField<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        crossfade_rtpc_type: Option<BankField<u8>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        associations: Option<BlendTable>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum BlendAssociationRecord {
    Decoded {
        child_id: BankField<u32>,
        points: BlendTable,
        reference_resolution: NodeReferenceStatus,
    },
    Malformed {
        child_id: BankField<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        points: Option<BlendTable>,
        reference_resolution: NodeReferenceStatus,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "layout", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum BlendPointRecord {
    Decoded {
        from: BlendFloatWord,
        to: BlendFloatWord,
        interpolation: BankField<u32>,
    },
    Malformed {
        from: BlendFloatWord,
        #[serde(skip_serializing_if = "Option::is_none")]
        to: Option<BlendFloatWord>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlendFloatWord {
    pub(crate) bits: BankField<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) finite_value: Option<f32>,
}
