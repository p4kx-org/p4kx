// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use super::{CONTAINER, Owner, TestResult};

pub(super) struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    pub(super) fn read(base: &Path) -> TestResult<Self> {
        Ok(Self {
            fields: read_rows(base.join("field-origins.jsonl"))?,
            uses: read_rows(base.join("origin-uses.jsonl"))?,
            sources: read_rows(base.join("origin-sources.jsonl"))?,
        })
    }

    pub(super) fn field(
        &self,
        stream: &str,
        ordinal: usize,
        path: &str,
        encrypted: bool,
    ) -> TestResult<&Value> {
        let ordinal = ordinal.to_string();
        let field = self
            .fields
            .iter()
            .find(|row| {
                row.get("type").and_then(Value::as_str) == Some("field")
                    && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some(stream)
                    && (stream != "records"
                        || row.pointer("/scope/stream/kind").and_then(Value::as_str)
                            == Some("audio"))
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.get("field").and_then(Value::as_str) == Some(path)
            })
            .ok_or_else(|| format!("immediate Audio origin {stream} {ordinal} {path}"))?;
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&Value::Bool(true)),
            "{path}: complete source uses"
        );
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&Value::Bool(encrypted)),
            "{path}: immediate marker"
        );
        Ok(field)
    }

    pub(super) fn require_owner(&self, receipt: &Value, owner: Owner) -> TestResult<Vec<&Value>> {
        let refs = receipt
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source-use refs")?;
        assert!(
            !refs.is_empty(),
            "actual WEM field has physical source uses"
        );
        let mut used = Vec::new();
        for reference in refs {
            let observation = self
                .uses
                .iter()
                .find(|row| row.get("id") == Some(reference))
                .ok_or("source-use row")?;
            let source = self.physical(observation.get("source").ok_or("source-use owner")?)?;
            assert_eq!(
                source.pointer("/path/value"),
                Some(&json!(owner.path)),
                "original leaf path"
            );
            assert_eq!(
                source.get("entryIndex"),
                Some(&json!(owner.entry)),
                "same-name physical Entry index"
            );
            assert_eq!(
                source.get("storedEncryption"),
                Some(&json!(owner.stored)),
                "stored leaf encryption"
            );
            self.check_parent(source, owner)?;
            used.push(observation);
        }
        Ok(used)
    }

    fn check_parent(&self, source: &Value, owner: Owner) -> TestResult {
        let ancestors = source
            .get("ancestors")
            .and_then(Value::as_array)
            .ok_or("physical ancestry")?;
        match (ancestors.as_slice(), owner.parent) {
            ([], None) => {}
            ([reference], Some(expected)) => {
                let parent = self.physical(reference)?;
                assert_eq!(
                    parent.get("entryIndex"),
                    Some(&json!(expected)),
                    "containing Entry index"
                );
                assert_eq!(
                    parent.pointer("/path/value"),
                    Some(&json!(CONTAINER)),
                    "original containing Entry path"
                );
                assert_eq!(
                    parent.get("storedEncryption"),
                    Some(&json!("encrypted")),
                    "actual containing Entry proof"
                );
            }
            _ => return Err("wrong physical ancestry".into()),
        }
        Ok(())
    }

    fn physical(&self, reference: &Value) -> TestResult<&Value> {
        self.sources
            .iter()
            .find(|row| row.get("id") == Some(reference))
            .ok_or_else(|| "physical source row".into())
    }
}

pub(super) fn read_rows<T: DeserializeOwned>(path: PathBuf) -> TestResult<Vec<T>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}
