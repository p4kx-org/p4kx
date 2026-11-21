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
pub struct ShopInventorySource {
    /// An owner-local index into `values`.
    pub(super) root: DecimalU64,
    pub(super) values: Vec<SourcedValue<JsonValue>>,
}

impl ShopInventorySource {
    /// Returns the local address of the original JSON root.
    #[must_use]
    pub const fn root(&self) -> DecimalU64 {
        self.root
    }

    /// Borrows the original value cells addressed by their local indices.
    #[must_use]
    pub fn values(&self) -> &[SourcedValue<JsonValue>] {
        &self.values
    }
}

/// One original non-null JSON value in a shop inventory document.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum JsonValue {
    /// A Boolean token.
    Boolean {
        /// The stored value.
        value: bool,
    },
    /// A number token.
    Number {
        /// The exact token, including all digits and its exponent.
        #[schemars(schema_with = "number_schema")]
        token: String,
    },
    /// A string token.
    String {
        /// The retained original string representation.
        value: JsonString,
    },
    /// An ordered array.
    Array {
        /// Local references to the original elements.
        elements: Vec<DecimalU64>,
    },
    /// An object with original member order and multiplicity.
    Object {
        /// Members, including repeated names.
        members: Vec<JsonMember>,
    },
}

/// The original string and its text protection category.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
#[non_exhaustive]
pub enum JsonString {
    /// A classified technical string.
    Public {
        /// The decoded text.
        text: String,
    },
    /// A protected original string.
    GameText {
        /// The decoded text.
        #[schemars(extend("x-gameText" = true))]
        text: String,
    },
    /// An original token that cannot be represented as a decoded string.
    Raw {
        /// The protected original token.
        #[schemars(extend("x-gameText" = true))]
        token: String,
    },
}

/// A named original member and the local address of its value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct JsonMember {
    pub(super) name: SourcedValue<JsonString>,
    /// An owner-local index into the same source arena.
    pub(super) value: DecimalU64,
}

impl JsonMember {
    /// Borrows the original member name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<JsonString> {
        &self.name
    }

    /// Returns the local address of the original value.
    #[must_use]
    pub const fn value(&self) -> DecimalU64 {
        self.value
    }
}

fn number_schema(_generator: &mut SchemaGenerator) -> Schema {
    json_schema!({
        "type": "string",
        "pattern": r"^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$",
        "not": { "pattern": "[^0-9eE+.-]" },
        "description": "Exact authored JSON number token with all digits, sign and exponent."
    })
}
