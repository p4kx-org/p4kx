// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::hirc_view::StateRange;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

pub(crate) type ObjectSourceSchema =
    DialogueSourceDocument<Vec<DialogueSourceRow>, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "DialogueObjectSourceFields")]
pub(crate) struct DialogueSourceDocument<Rows, Inventory> {
    pub(crate) kind: DialogueSourceKind,
    pub(crate) layout: SourcedValue<DialogueSourceLayout>,
    pub(crate) version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<SourcedValue<String>>,
    pub(crate) fields: DialogueSummary,
    #[schemars(with = "Vec<DialogueSourceRow>")]
    pub(crate) rows: Rows,
    #[schemars(with = "SourceInventory")]
    pub(crate) source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueSourceKind {
    DialogueEvent,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueSourceLayout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DialogueSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) probability: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) argument_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) checked_arguments: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) checked_argument_types: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree_size: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) mode_code: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tree: Option<DialogueTreeSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) properties: Option<DialoguePropertyExtent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ranged_modifiers: Option<DialoguePropertyExtent>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DialogueTreeSummary {
    pub(crate) range: SourcedValue<StateRange>,
    pub(crate) coverage: SourcedValue<DialogueTreeStatus>,
    pub(crate) complete_nodes: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) partial_node_range: Option<SourcedValue<StateRange>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueTreeStatus {
    Complete,
    Truncated,
    Misaligned,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DialoguePropertyExtent {
    pub(crate) declared_count: SourcedValue<u8>,
    pub(crate) checked_ids: SourcedValue<u8>,
    pub(crate) checked_words: SourcedValue<u16>,
    pub(crate) complete: SourcedValue<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum DialogueSourceRow {
    Argument {
        ordinal: SourcedValue<u32>,
        group_id: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        group_type_code: Option<SourcedValue<u8>>,
    },
    Node {
        ordinal: SourcedValue<u32>,
        range: SourcedValue<StateRange>,
        key: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        link_word: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        link_interpretation: Option<SourcedValue<DialogueLinkInterpretation>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        weight: Option<SourcedValue<u16>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        probability: Option<SourcedValue<u16>>,
    },
    Property {
        list: SourcedValue<DialoguePropertyKind>,
        ordinal: SourcedValue<u8>,
        property_id: SourcedValue<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        scalar_bits: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        minimum_bits: Option<SourcedValue<u32>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maximum_bits: Option<SourcedValue<u32>>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialogueLinkInterpretation {
    Undetermined,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DialoguePropertyKind {
    Properties,
    RangedModifiers,
}
