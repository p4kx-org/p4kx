// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::ops::Range;
use std::path::Path;

use serde_json::{Value, json};

use super::ActionTestResult;

pub(super) struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    pub(super) fn read(base: &Path) -> ActionTestResult<Self> {
        Ok(Self {
            fields: rows(base, "field-origins.jsonl")?,
            uses: rows(base, "origin-uses.jsonl")?,
            sources: rows(base, "origin-sources.jsonl")?,
        })
    }

    pub(super) fn uses(
        &self,
        record: usize,
        owner: usize,
        path: &str,
    ) -> ActionTestResult<Vec<&Value>> {
        let ordinal = record.to_string();
        let field = self
            .fields
            .iter()
            .find(|row| {
                row.get("field").and_then(Value::as_str) == Some(path)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("audio")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
            })
            .ok_or("immediate original HIRC field receipt")?;
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(true)),
            "all actual HIRC inputs are known for {path}"
        );
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(owner == 1)),
            "per-value marker belongs to the original bank"
        );
        let references = field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("actual source-use refs")?;
        references
            .iter()
            .map(|reference| self.find_use(reference, owner))
            .collect()
    }

    fn find_use(&self, reference: &Value, owner: usize) -> ActionTestResult<&Value> {
        let source_use = self
            .uses
            .iter()
            .find(|row| row.get("id") == Some(reference))
            .ok_or("original source use")?;
        let physical = source_use.get("source").ok_or("source-use owner")?;
        let source = self
            .sources
            .iter()
            .find(|row| row.get("id") == Some(physical))
            .ok_or("original physical bank")?;
        assert_eq!(
            source.get("entryIndex"),
            Some(&json!(owner)),
            "equal short IDs cannot bind another physical bank"
        );
        Ok(source_use)
    }
}

pub(super) fn has_exact(uses: &[&Value], name: &str, role: &str, range: &Range<usize>) -> bool {
    let location = json!({
        "type":"subresource","namespace":"bnkHircField",
        "indices":[range.start.to_string(), range.end.to_string()],"field":name
    });
    uses.iter().any(|used| {
        used.get("role").and_then(Value::as_str) == Some(role)
            && used.get("location") == Some(&location)
    })
}

pub(super) fn has_field(uses: &[&Value], name: &str) -> bool {
    uses.iter()
        .any(|used| used.pointer("/location/field").and_then(Value::as_str) == Some(name))
}

fn rows(directory: &Path, file: &str) -> ActionTestResult<Vec<Value>> {
    fs::read_to_string(directory.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
