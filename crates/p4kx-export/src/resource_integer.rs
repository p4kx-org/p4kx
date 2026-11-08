// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Serialize, Serializer};

/// A signed source Int64 encoded as range-checked canonical decimal text.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResourceInt64(i64);

impl ResourceInt64 {
    pub(crate) const fn new(value: i64) -> Self {
        Self(value)
    }
}

impl Serialize for ResourceInt64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl JsonSchema for ResourceInt64 {
    fn schema_name() -> Cow<'static, str> {
        "ResourceInt64".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        let positive = bounded(&i64::MAX.to_string());
        let negative = bounded(&i64::MIN.unsigned_abs().to_string());
        json_schema!({
            "type": "string", "maxLength": 20,
            "pattern": format!("^(?:0|{positive}|-(?:{negative}))$"),
            "not": {"pattern": "[^0-9-]"},
            "description": "Canonical signed 64-bit decimal text."
        })
    }
}

fn bounded(maximum: &str) -> String {
    let shorter = maximum.len() - 2;
    let mut alternatives = vec![format!("[1-9][0-9]{{0,{shorter}}}")];
    let mut prefix = String::new();
    for (position, digit) in maximum.bytes().enumerate() {
        let minimum = if position == 0 { b'1' } else { b'0' };
        if digit > minimum {
            let remaining = maximum.len() - position - 1;
            alternatives.push(format!(
                "{prefix}[{}-{}][0-9]{{{remaining}}}",
                char::from(minimum),
                char::from(digit - 1),
            ));
        }
        prefix.push(char::from(digit));
    }
    alternatives.push(maximum.to_owned());
    format!("(?:{})", alternatives.join("|"))
}
