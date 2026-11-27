// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use p4kx_extraction::DecimalU64;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::support::{Fixture, TestResult};
use crate::origin_contract::OriginCoverage;
use crate::origin_inventory_wire::{Encryption, Role, Scope};
use crate::{Artifact, hashing};

#[derive(Debug, Clone, Copy)]
pub(super) enum Target {
    Leaf,
    UnknownAncestor,
    Standalone,
    WholeArchive,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum UseSet {
    FieldContributor,
    UnusedReference,
}

#[derive(Clone, Copy)]
pub(super) struct Witness {
    pub(super) target: Target,
    pub(super) scope: Scope,
    pub(super) captured: Encryption,
    pub(super) forged: Encryption,
    pub(super) use_set: UseSet,
}

pub(super) struct Snapshot {
    pub(super) sources: Vec<Value>,
    pub(super) uses: Vec<Value>,
    fields: Vec<Value>,
}

impl Snapshot {
    pub(super) fn load(fixture: &Fixture) -> Result<Self, Box<dyn std::error::Error>> {
        let receipts = fixture
            .index
            .field_origins
            .as_ref()
            .ok_or("current receipts")?;
        Ok(Self {
            sources: super::support::rows(&fixture.root.join(&receipts.sources.path))?,
            uses: super::support::rows(&fixture.root.join(&receipts.uses.path))?,
            fields: super::support::rows(&fixture.root.join(&receipts.fields.path))?,
        })
    }

    pub(super) fn source(&self, target: Target) -> Result<&Value, Box<dyn std::error::Error>> {
        let kind = match target {
            Target::Standalone => Some("file"),
            Target::WholeArchive => Some("archiveFile"),
            Target::Leaf | Target::UnknownAncestor => None,
        };
        if let Some(kind) = kind {
            return self
                .sources
                .iter()
                .find(|row| row.get("kind").and_then(Value::as_str) == Some(kind))
                .ok_or_else(|| "original physical source kind".into());
        }
        let leaf = self
            .sources
            .iter()
            .find(|row| row.pointer("/path/value").and_then(Value::as_str) == Some("leaf.ext"))
            .ok_or("original physical leaf")?;
        if matches!(target, Target::Leaf) {
            return Ok(leaf);
        }
        let ancestors = leaf
            .get("ancestors")
            .and_then(Value::as_array)
            .ok_or("actual ancestors")?;
        self.sources
            .iter()
            .find(|row| {
                row.get("kind").and_then(Value::as_str) == Some("unknown")
                    && ancestors
                        .iter()
                        .any(|reference| Some(reference) == row.get("id"))
            })
            .ok_or_else(|| "actual unknown physical ancestor".into())
    }

    pub(super) fn forge(&mut self, witness: Witness) -> TestResult {
        let source = self
            .source(witness.target)?
            .get("id")
            .and_then(Value::as_str)
            .ok_or("actual physical source reference")?
            .to_owned();
        let position = self.find_use(&source, witness)?;
        let used = self.uses.get_mut(position).ok_or("actual use")?;
        let captured: Encryption = serde_json::from_value(
            used.get("effectiveEncryption")
                .ok_or("captured domain evidence")?
                .clone(),
        )?;
        assert_eq!(
            captured, witness.captured,
            "the genuine producer establishes the selected domain"
        );
        *used
            .get_mut("effectiveEncryption")
            .ok_or("captured domain evidence")? = Value::String(state_name(witness.forged).into());
        Ok(())
    }

    fn find_use(
        &self,
        source: &str,
        witness: Witness,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        for (position, used) in self.uses.iter().enumerate() {
            if used.get("source").and_then(Value::as_str) != Some(source) {
                continue;
            }
            let scope: Scope =
                serde_json::from_value(used.get("scope").ok_or("actual scope")?.clone())?;
            let role: Role =
                serde_json::from_value(used.get("role").ok_or("actual role")?.clone())?;
            if scope != witness.scope {
                continue;
            }
            let reference = used
                .get("id")
                .and_then(Value::as_str)
                .ok_or("actual use reference")?;
            let referenced = self.fields.iter().any(|row| references(row, reference));
            let matches = match witness.use_set {
                UseSet::FieldContributor => role.contributes() && referenced,
                UseSet::UnusedReference => role == Role::Reference && !referenced,
            };
            if matches {
                return Ok(position);
            }
        }
        Err("genuine producer use in the selected domain".into())
    }

    pub(super) fn publish_forgery(&mut self, fixture: &mut Fixture) -> TestResult {
        let evidence = use_states(&self.uses)?;
        for row in self.sources.iter_mut().chain(&mut self.fields) {
            rewrite_origins(row, &evidence)?;
        }
        coverage(&mut self.fields)?;
        let receipts = fixture
            .index
            .field_origins
            .as_mut()
            .ok_or("current receipts")?;
        for summary in &mut receipts.streams {
            let stream = serde_json::to_value(summary.stream)?;
            let matching = |row: &&Value| row.pointer("/scope/stream") == Some(&stream);
            summary.occurrences = DecimalU64::new(u64::try_from(
                self.fields
                    .iter()
                    .filter(matching)
                    .filter(|row| row.get("type").and_then(Value::as_str) == Some("occurrence"))
                    .count(),
            )?);
            summary.fields = DecimalU64::new(u64::try_from(
                self.fields
                    .iter()
                    .filter(matching)
                    .filter(|row| row.get("type").and_then(Value::as_str) == Some("field"))
                    .count(),
            )?);
        }
        let eof = self.fields.last_mut().ok_or("terminal EOF")?;
        assert_eq!(
            eof.get("type").and_then(Value::as_str),
            Some("eof"),
            "genuine terminal EOF"
        );
        *eof.get_mut("streams").ok_or("EOF multiplicities")? =
            serde_json::to_value(&receipts.streams)?;
        write_rows(&fixture.root, &mut receipts.sources, &self.sources)?;
        write_rows(&fixture.root, &mut receipts.uses, &self.uses)?;
        write_rows(&fixture.root, &mut receipts.fields, &self.fields)?;
        fixture.rebind()
    }
}

pub(super) fn state_name(state: Encryption) -> &'static str {
    match state {
        Encryption::Encrypted => "encrypted",
        Encryption::Unencrypted => "unencrypted",
        Encryption::Unknown => "unknown",
    }
}

fn references(row: &Value, reference: &str) -> bool {
    row.get("sourceUsesRefs")
        .and_then(Value::as_array)
        .is_some_and(|references| {
            references
                .iter()
                .any(|value| value.as_str() == Some(reference))
        })
}

fn use_states(rows: &[Value]) -> Result<BTreeMap<String, Encryption>, Box<dyn std::error::Error>> {
    let mut states = BTreeMap::new();
    for row in rows {
        let reference = row
            .get("id")
            .and_then(Value::as_str)
            .ok_or("original use id")?;
        let state = serde_json::from_value(
            row.get("effectiveEncryption")
                .ok_or("captured state")?
                .clone(),
        )?;
        assert!(
            states.insert(reference.to_owned(), state).is_none(),
            "original use ids are unique"
        );
    }
    Ok(states)
}

fn rewrite_origins(value: &mut Value, states: &BTreeMap<String, Encryption>) -> TestResult {
    match value {
        Value::Object(object) => {
            if object.contains_key("sourceUsesRefs") {
                let complete = object
                    .get("sourceSetComplete")
                    .and_then(Value::as_bool)
                    .ok_or("actual completeness")?;
                let references = object
                    .get("sourceUsesRefs")
                    .and_then(Value::as_array)
                    .ok_or("actual contribution references")?;
                let mut encrypted = false;
                let mut unknown = !complete;
                for reference in references {
                    let reference = reference.as_str().ok_or("original reference text")?;
                    match states.get(reference).ok_or("closed actual reference")? {
                        Encryption::Encrypted => encrypted = true,
                        Encryption::Unknown => unknown = true,
                        Encryption::Unencrypted => {}
                    }
                }
                let marker = if encrypted {
                    Some(true)
                } else if unknown {
                    None
                } else {
                    Some(false)
                };
                object.insert(
                    "containsEncryptedSource".into(),
                    serde_json::to_value(marker)?,
                );
            }
            for child in object.values_mut() {
                rewrite_origins(child, states)?;
            }
        }
        Value::Array(array) => {
            for child in array {
                rewrite_origins(child, states)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn coverage(rows: &mut [Value]) -> TestResult {
    let mut counts = Counts::default();
    for row in rows {
        match row.get("type").and_then(Value::as_str) {
            Some("occurrence") => counts = Counts::default(),
            Some("field") => counts.observe(row)?,
            Some("end") => {
                *row.get_mut("coverage").ok_or("actual End coverage")? =
                    serde_json::to_value(counts.wire())?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Default)]
struct Counts {
    fields: u64,
    complete: u64,
    incomplete: u64,
    unknown: u64,
    encrypted: u64,
}

impl Counts {
    fn observe(&mut self, row: &Value) -> TestResult {
        increment(&mut self.fields)?;
        if row
            .get("sourceSetComplete")
            .and_then(Value::as_bool)
            .ok_or("actual completeness")?
        {
            increment(&mut self.complete)?;
        } else {
            increment(&mut self.incomplete)?;
        }
        match row
            .get("containsEncryptedSource")
            .ok_or("required marker")?
            .as_bool()
        {
            Some(true) => increment(&mut self.encrypted)?,
            None => increment(&mut self.unknown)?,
            Some(false) => {}
        }
        Ok(())
    }

    const fn wire(&self) -> OriginCoverage {
        OriginCoverage {
            fields: DecimalU64::new(self.fields),
            complete: DecimalU64::new(self.complete),
            incomplete: DecimalU64::new(self.incomplete),
            unknown: DecimalU64::new(self.unknown),
            encrypted: DecimalU64::new(self.encrypted),
        }
    }
}

fn increment(value: &mut u64) -> TestResult {
    *value = value.checked_add(1).ok_or("synthetic counter overflow")?;
    Ok(())
}

fn write_rows(root: &Path, artifact: &mut Artifact, rows: &[Value]) -> TestResult {
    let mut bytes = Vec::new();
    for row in rows {
        serde_json::to_writer(&mut bytes, row)?;
        bytes.push(b'\n');
    }
    fs::write(root.join(&artifact.path), &bytes)?;
    artifact.size = u64::try_from(bytes.len())?;
    artifact.digest.hex = hashing::hex(Sha256::digest(&bytes).into());
    Ok(())
}
