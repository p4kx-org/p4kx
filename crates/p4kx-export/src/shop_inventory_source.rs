// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::Serialize;

use crate::schema_fields::every_field_required;

/// The complete ordered JSON tokens of one inventory file.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShopInventorySource {
    /// An owner-local index into `values`.
    pub(super) root: DecimalU64,
    pub(super) values: Vec<SourcedValue<JsonValue>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum JsonValue {
    Boolean {
        value: bool,
    },
    Number {
        #[schemars(schema_with = "number_schema")]
        token: String,
    },
    String {
        value: JsonString,
    },
    Array {
        elements: Vec<DecimalU64>,
    },
    Object {
        members: Vec<JsonMember>,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(super) enum JsonString {
    Public {
        text: String,
    },
    GameText {
        #[schemars(extend("x-gameText" = true))]
        text: String,
    },
    Raw {
        #[schemars(extend("x-gameText" = true))]
        token: String,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct JsonMember {
    pub(super) name: SourcedValue<JsonString>,
    /// An owner-local index into the same source arena.
    pub(super) value: DecimalU64,
}

fn number_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "string",
        "pattern": r"^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$",
        "not": { "pattern": "[^0-9eE+.-]" },
        "description": "Exact authored JSON number token with all digits, sign and exponent."
    })
}
