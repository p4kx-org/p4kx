// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::{MediaError, SourceRange, schema_merge};

pub(super) const TABLE: &str = "model-node-table";
pub(super) const NODE: &str = "model-node";
pub(super) const PROPERTY: &str = "model-node-property";
pub(super) const MATERIAL: &str = "model-node-material-index";

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum NmcRecord<'a> {
    NodeTable(Table),
    Node(Node<'a>),
    NodeProperty(Property<'a>),
    NodeMaterialIndex(MaterialIndex),
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "disposition", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Table {
    Measured {
        nodes: usize,
        names: SourceRange,
        properties: SourceRange,
        declared_property_bytes: u32,
        material_indices: SourceRange,
    },
    UnsupportedVersion,
    Malformed,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Node<'a> {
    pub(super) stored_id: u32,
    pub(super) parent_word: u16,
    pub(super) geometry_type: u16,
    pub(super) helper: bool,
    pub(super) local_matrix_bits: [[u32; 4]; 3],
    pub(super) companion_matrix_bits: [[u32; 4]; 3],
    pub(super) scale_bits: [u32; 3],
    pub(super) name: Option<Text<'a>>,
    pub(super) property_entry: Option<SourceRange>,
    pub(super) property_entry_terminated: Option<bool>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Text<'a> {
    pub(super) utf8: Option<&'a str>,
    pub(super) display_name_utf8: Option<&'a str>,
    pub(super) range: SourceRange,
    pub(super) terminated: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Property<'a> {
    pub(super) raw_utf8: Option<&'a str>,
    pub(super) range: SourceRange,
    pub(super) assignment: Option<Assignment<'a>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct Assignment<'a> {
    pub(super) key: Option<&'a str>,
    pub(super) value: Option<&'a str>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MaterialIndex {
    pub(super) word: u16,
}

const SCHEMA: &str = "ModelNmcRecord";
pub(super) const SCHEMA_ROWS: [(&str, &str, usize, &str); 4] = [
    (TABLE, "node-table", 1, SCHEMA),
    (NODE, "node", 2, SCHEMA),
    (PROPERTY, "node-property", 3, SCHEMA),
    (MATERIAL, "node-material-index", 2, SCHEMA),
];

pub(super) fn insert_schema(base: &mut serde_json::Value) -> Result<(), MediaError> {
    schema_merge::insert_payload(
        base,
        SCHEMA,
        schema_merge::serialized_payload::<NmcRecord<'static>>(),
    )
}
