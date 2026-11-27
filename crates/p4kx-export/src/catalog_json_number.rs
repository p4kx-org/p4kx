// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::Serialize;

/// An original JSON number with its full decimal spelling.
#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct CatalogJsonNumber(pub(crate) String);

impl CatalogJsonNumber {
    /// Borrows the exact original decimal token.
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl JsonSchema for CatalogJsonNumber {
    fn schema_name() -> Cow<'static, str> {
        "CatalogJsonNumber".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": r"^-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?$",
            "not": { "pattern": "[^0-9eE+.-]" },
            "description": "Exact authored JSON number token with all digits, sign and exponent."
        })
    }
}
