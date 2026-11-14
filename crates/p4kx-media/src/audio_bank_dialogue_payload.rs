// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_property_payload::NodePropertyListRecord;
use crate::audio_bank_payload::{BankEntryRange, BankField};

pub(crate) const SUMMARY: &str = "audio-bank-dialogue-event";
pub(crate) const ARGUMENT: &str = "audio-bank-dialogue-argument";
pub(crate) const NODE: &str = "audio-bank-dialogue-node";

#[derive(Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", deny_unknown_fields)]
pub(crate) enum DialogueEventPayload<'row> {
    #[serde(rename = "bank-dialogue-event")]
    Event(&'row NodeDeclaration<DialogueEventObservations>),
    #[serde(rename = "bank-dialogue-argument")]
    Argument(&'row DialogueArgumentRecord),
    #[serde(rename = "bank-dialogue-node")]
    Node(&'row DialogueNodeRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DialogueEventObservations {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) probability: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) argument_count: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) checked_arguments: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) checked_argument_types: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree_size: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode_code: Option<BankField<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode: Option<DialogueDecisionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree_range: Option<BankEntryRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree_coverage: Option<DialogueTreeCoverage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) complete_nodes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) partial_node_bytes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) partial_node_range: Option<BankEntryRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) properties: Option<NodePropertyListRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ranged_modifiers: Option<NodePropertyListRecord>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueDecisionMode {
    BestMatch,
    Weighted,
    Unknown,
}

impl From<u8> for DialogueDecisionMode {
    fn from(code: u8) -> Self {
        match code {
            0 => Self::BestMatch,
            1 => Self::Weighted,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueTreeCoverage {
    Complete,
    Truncated,
    Misaligned,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DialogueArgumentRecord {
    #[serde(rename = "group_id")]
    pub(crate) id: BankField<u32>,
    #[serde(rename = "group_type_code", skip_serializing_if = "Option::is_none")]
    pub(crate) type_code: Option<BankField<u8>>,
    #[serde(rename = "group_type", skip_serializing_if = "Option::is_none")]
    pub(crate) kind: Option<DialogueGroupType>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueGroupType {
    Switch,
    State,
    Unknown,
}

impl From<u8> for DialogueGroupType {
    fn from(code: u8) -> Self {
        match code {
            0 => Self::Switch,
            1 => Self::State,
            _ => Self::Unknown,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DialogueNodeRecord {
    pub(crate) layout: DialogueNodeLayout,
    pub(crate) range: BankEntryRange,
    pub(crate) key: BankField<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) link_word: Option<BankField<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) link_interpretation: Option<DialogueLinkInterpretation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) weight: Option<BankField<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) probability: Option<BankField<u16>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueNodeLayout {
    Decoded,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueLinkInterpretation {
    Undetermined,
}
