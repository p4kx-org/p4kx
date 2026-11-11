// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::BankTestResult;

pub(super) struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    pub(super) fn read(directory: &Path) -> BankTestResult<Self> {
        Ok(Self {
            fields: rows(directory, "field-origins.jsonl")?,
            uses: rows(directory, "origin-uses.jsonl")?,
            sources: rows(directory, "origin-sources.jsonl")?,
        })
    }

    pub(super) fn media(
        &self,
        record: usize,
        owner: usize,
        start: usize,
        prefetch: bool,
    ) -> BankTestResult {
        let class = self.use_rows(record, owner, "/data/payload/data/class")?;
        assert!(
            has_header(&class, start, "signature"),
            "class references its actual selected physical embedded signature"
        );
        assert_eq!(
            has_header(&class, start, "form"),
            prefetch,
            "only the RIFF class consults its form"
        );
        assert!(
            !has_header(&class, start, "riffSize"),
            "class decision skips the RIFF size word"
        );

        let length = self.use_rows(record, owner, "/data/payload/data/declared_wem_length")?;
        assert!(
            has_header(&length, start, "signature"),
            "length consulted the actual signature"
        );
        assert_eq!(
            has_header(&length, start, "riffSize"),
            prefetch,
            "only RIFF length consults its size word"
        );
        Ok(())
    }

    fn use_rows(&self, record: usize, owner: usize, path: &str) -> BankTestResult<Vec<&Value>> {
        let position = record.to_string();
        let receipt = self
            .fields
            .iter()
            .find(|row| {
                row.get("type").and_then(Value::as_str) == Some("field")
                    && row.get("field").and_then(Value::as_str) == Some(path)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("audio")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(position.as_str())
            })
            .ok_or("actual per-field Audio origin")?;
        assert_eq!(
            receipt.get("sourceSetComplete"),
            Some(&json!(true)),
            "complete chosen source inputs"
        );
        assert_eq!(
            receipt.get("containsEncryptedSource"),
            Some(&json!(owner == 1)),
            "actual physical owner supplies the immediate encryption marker"
        );
        let references = receipt
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source-use references")?;
        references
            .iter()
            .map(|reference| self.lookup_owner(reference, owner))
            .collect()
    }

    fn lookup_owner(&self, reference: &Value, owner: usize) -> BankTestResult<&Value> {
        let used = self
            .uses
            .iter()
            .find(|row| row.get("id") == Some(reference))
            .ok_or("actual source use")?;
        let source = used.get("source").ok_or("source-use owner")?;
        let physical = self
            .sources
            .iter()
            .find(|row| row.get("id") == Some(source))
            .ok_or("physical source")?;
        assert_eq!(
            physical.get("entryIndex"),
            Some(&json!(owner)),
            "no cross-bank rebinding of duplicate short IDs"
        );
        Ok(used)
    }
}

fn has_header(uses: &[&Value], start: usize, field: &str) -> bool {
    let expected = json!({
        "type": "subresource",
        "namespace": "bnkMediaHeader",
        "indices": [start.to_string()],
        "field": field
    });
    uses.iter()
        .any(|row| row.get("location") == Some(&expected))
}

fn rows(directory: &Path, file: &str) -> BankTestResult<Vec<Value>> {
    fs::read_to_string(directory.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
