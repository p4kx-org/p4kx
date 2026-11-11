// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use super::TestResult;

pub(super) struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    pub(super) fn read(base: &Path) -> TestResult<Self> {
        Ok(Self {
            fields: read(base, "field-origins.jsonl")?,
            uses: read(base, "origin-uses.jsonl")?,
            sources: read(base, "origin-sources.jsonl")?,
        })
    }

    pub(super) fn for_field(
        &self,
        record: usize,
        owner: usize,
        path: &str,
    ) -> TestResult<Vec<&Value>> {
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
            .ok_or("immediate annotation field origin")?;
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(owner == 1)),
            "actual physical input encryption for {path}"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(true)),
            "actual field input set for {path}"
        );
        let refs = field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("actual annotation use references")?;
        refs.iter()
            .map(|reference| self.find(reference, owner))
            .collect()
    }

    fn find(&self, reference: &Value, owner: usize) -> TestResult<&Value> {
        let observed = self
            .uses
            .iter()
            .find(|row| row.get("id") == Some(reference))
            .ok_or("annotation source use")?;
        let owner_ref = observed.get("source").ok_or("annotation source owner")?;
        let source = self
            .sources
            .iter()
            .find(|row| row.get("id") == Some(owner_ref))
            .ok_or("actual physical WEM")?;
        assert_eq!(
            source.get("entryIndex"),
            Some(&json!(owner)),
            "same-name WEMs retain distinct evidence"
        );
        Ok(observed)
    }
}

pub(super) fn exact(
    uses: &[&Value],
    namespace: &str,
    field: &str,
    start: usize,
    end: usize,
) -> bool {
    let site = json!({"type":"subresource","namespace":namespace,"indices":[start.to_string(),end.to_string()],"field":field});
    uses.iter().any(|used| used.get("location") == Some(&site))
}

pub(super) fn contains(uses: &[&Value], field: &str) -> bool {
    uses.iter()
        .any(|used| used.pointer("/location/field").and_then(Value::as_str) == Some(field))
}

fn read(base: &Path, file: &str) -> TestResult<Vec<Value>> {
    fs::read_to_string(base.join(file))?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
