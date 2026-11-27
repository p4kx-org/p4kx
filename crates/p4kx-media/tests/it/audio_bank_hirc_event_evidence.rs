// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::ops::Range;
use std::path::Path;

use serde_json::{Value, json};

use super::TestResult;

pub(super) struct Evidence {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Evidence {
    pub(super) fn read(base: &Path) -> TestResult<Self> {
        Ok(Self {
            fields: read(base, "field-origins.jsonl")?,
            uses: read(base, "origin-uses.jsonl")?,
            sources: read(base, "origin-sources.jsonl")?,
        })
    }

    pub(super) fn exact(
        &self,
        record: usize,
        owner: usize,
        path: &str,
        name: &str,
        extent: &Range<usize>,
    ) -> TestResult {
        let position = record.to_string();
        let field = self
            .fields
            .iter()
            .find(|row| {
                row.get("field").and_then(Value::as_str) == Some(path)
                    && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("audio")
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(position.as_str())
            })
            .ok_or("original Event field receipt")?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(owner == 1)),
            "actual owner encryption for {path}"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(true)),
            "actual original uses known for {path}"
        );
        let references = field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("Event use references")?;
        let site = json!({"type":"subresource","namespace":"bnkHircField","indices":[extent.start.to_string(),extent.end.to_string()],"field":name});
        let mut found = false;
        for reference in references {
            let used = self
                .uses
                .iter()
                .find(|row| row.get("id") == Some(reference))
                .ok_or("original Event use")?;
            self.check_owner(used, owner)?;
            found |= used.get("location") == Some(&site)
                && used.get("role").and_then(Value::as_str) == Some("payload");
        }
        assert!(
            found,
            "{path} points at the exact original word or varint bytes"
        );
        Ok(())
    }

    fn check_owner(&self, used: &Value, owner: usize) -> TestResult {
        let physical = used.get("source").ok_or("physical Event owner")?;
        let source = self
            .sources
            .iter()
            .find(|row| row.get("id") == Some(physical))
            .ok_or("original source")?;
        assert_eq!(
            source.get("entryIndex"),
            Some(&json!(owner)),
            "duplicate Event IDs bind to their actual Archive Entry"
        );
        Ok(())
    }
}

fn read(base: &Path, file: &str) -> TestResult<Vec<Value>> {
    fs::read_to_string(base.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
