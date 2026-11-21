// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::Deserializer;
use serde::de::{SeqAccess, Visitor};
use serde_json::Value;

use crate::real_archive_texts::{legacy, text_fields};

pub(super) fn read<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    deserializer.deserialize_seq(LoadoutRows)
}

struct LoadoutRows;

impl<'de> Visitor<'de> for LoadoutRows {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a catalog loadout array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let fields = text_fields();
        let mut retained = Vec::new();
        while let Some(row) = sequence.next_element::<Value>()? {
            retained.push(legacy::retain_candidates(row, &fields).unwrap_or_default());
        }
        Ok(Value::Array(retained))
    }
}
