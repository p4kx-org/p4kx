// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
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

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string", "pattern": DecimalPattern.to_string(), "maxLength": 20,
            "not": { "pattern": "[^0-9]" },
            "description": "Canonical unsigned 64-bit decimal text, from 0 through 18446744073709551615."
        })
    }
}

struct DecimalPattern;

impl fmt::Display for DecimalPattern {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let maximum = u64::MAX.to_string();
        formatter.write_str("^(?:0|[1-9][0-9]{0,18}")?;
        let mut prefix = String::new();
        for (position, digit) in maximum.bytes().enumerate() {
            let minimum = if position == 0 { b'1' } else { b'0' };
            if digit > minimum {
                let remaining = maximum.len() - position - 1;
                write!(
                    formatter,
                    "|{prefix}[{}-{}][0-9]{{{remaining}}}",
                    char::from(minimum),
                    char::from(digit - 1)
                )?;
            }
            prefix.push(char::from(digit));
        }
        write!(formatter, "|{maximum})$")
    }
}
