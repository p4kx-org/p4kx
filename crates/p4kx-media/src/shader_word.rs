// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::DecimalU64;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

/// Exact unsigned 64-bit word represented as canonical decimal text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ShaderWord(u64);

impl ShaderWord {
    /// Retains all bits of one interpreted source word without JSON number conversion.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the exact interpreted word.
    #[must_use]
    pub const fn value(&self) -> u64 {
        self.0
    }
}

impl TryFrom<String> for ShaderWord {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let number = value
            .parse::<u64>()
            .map_err(|_| "invalid unsigned 64-bit decimal word")?;
        if number.to_string() != value {
            return Err("noncanonical unsigned 64-bit decimal word");
        }
        Ok(Self(number))
    }
}

impl From<ShaderWord> for String {
    fn from(word: ShaderWord) -> Self {
        word.0.to_string()
    }
}

impl JsonSchema for ShaderWord {
    fn schema_name() -> Cow<'static, str> {
        "ShaderWord".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        DecimalU64::json_schema(generator)
    }
}
