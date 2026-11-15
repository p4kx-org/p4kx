// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

pub(super) struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    pub(super) fn read(base: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            fields: read_jsonl(&base.join("field-origins.jsonl"))?,
            uses: read_jsonl(&base.join("origin-uses.jsonl"))?,
            sources: read_jsonl(&base.join("origin-sources.jsonl"))?,
        })
    }

    pub(super) fn proof(
        &self,
        ordinal: usize,
        field: &str,
        source: usize,
        encrypted: bool,
    ) -> Result<&Value, Box<dyn Error>> {
        let receipt = self.marker(ordinal, field, encrypted)?;
        let entry = self
            .sources
            .iter()
            .find(|row| row.get("entryIndex") == Some(&json!(source)))
            .and_then(|row| row.get("id"))
            .ok_or("original source Entry")?;
        let references = receipt
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("inline original source-use references")?;
        let [reference] = references.as_slice() else {
            return Err("field has exactly one original selector".into());
        };
        self.uses
            .iter()
            .find(|row| {
                row.get("id") == Some(reference)
                    && row.get("source") == Some(entry)
                    && row.get("scope").and_then(Value::as_str) == Some("payload")
                    && row.get("role").and_then(Value::as_str) == Some("selector")
            })
            .ok_or_else(|| "verified original source selector".into())
    }

    pub(super) fn marker(
        &self,
        ordinal: usize,
        field: &str,
        encrypted: bool,
    ) -> Result<&Value, Box<dyn Error>> {
        let ordinal = ordinal.to_string();
        let receipt = self
            .fields
            .iter()
            .find(|row| {
                row.pointer("/scope/stream/kind").and_then(Value::as_str)
                    == Some("animation-definition")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.get("field").and_then(Value::as_str) == Some(field)
            })
            .ok_or("immediate field receipt")?;
        assert_eq!(
            receipt.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "authored field source encryption"
        );
        assert_eq!(
            receipt.get("sourceSetComplete"),
            Some(&json!(true)),
            "original contributing selectors retained"
        );
        Ok(receipt)
    }
}

fn read_jsonl(path: &Path) -> Result<Vec<Value>, Box<dyn Error>> {
    fs::read_to_string(path)?
        .lines()
        .map(|row| serde_json::from_str(row).map_err(Into::into))
        .collect()
}
