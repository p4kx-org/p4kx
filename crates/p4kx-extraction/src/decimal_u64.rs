// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::fmt;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Serialize, Serializer};

/// An unsigned 64-bit source integer encoded as a canonical decimal JSON string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecimalU64(u64);

impl DecimalU64 {
    /// Retains the complete source integer without a floating-point conversion.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the original unsigned source integer.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl Serialize for DecimalU64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl JsonSchema for DecimalU64 {
    fn schema_name() -> Cow<'static, str> {
        "DecimalU64".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string", "pattern": DecimalPattern.to_string(), "maxLength": 20,
            "not": {"pattern": "[^0-9]"},
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
