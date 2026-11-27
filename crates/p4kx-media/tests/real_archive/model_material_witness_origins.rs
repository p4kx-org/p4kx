// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

use super::selection::{Family, Sample};
use crate::TestResult;
use crate::archive_replay::Lines;
use crate::shader_accounting::require;

pub(super) fn verify(
    base: &Path,
    ordinal: usize,
    sample: &Sample<'_>,
    family: Family,
) -> TestResult {
    let pointer = format!("/data{}/original/value", family.reference());
    let resolution = format!("/data{}/resolution/status", family.reference());
    let evidence = Inventory {
        sources: indexed(Lines::open(&base.join("origin-sources.jsonl"))?)?,
        uses: indexed(Lines::open(&base.join("origin-uses.jsonl"))?)?,
    };
    let original = field(base, ordinal, &pointer)?;
    require(
        original.encryption == Some(sample.entry.is_encrypted()),
        "authored marker must agree with original payload",
    )?;
    evidence.authored(&original.uses, sample)?;
    let resolved = field(base, ordinal, &resolution)?;
    evidence.marker(&resolved)?;
    evidence.directory(&resolved.uses, 1, sample.material.path())?;
    if let Some(binding) = &sample.binding
        && sample.binding_used
    {
        evidence.directory(&resolved.uses, 2, binding.target.path())?;
    }
    Ok(())
}

struct Inventory {
    sources: BTreeMap<String, Value>,
    uses: BTreeMap<String, Value>,
}

impl Inventory {
    fn authored(&self, refs: &[String], sample: &Sample<'_>) -> TestResult {
        let [reference] = refs else {
            return Err("original spelling should have exactly one physical source use".into());
        };
        let (usage, source) = self.witness(reference)?;
        let location = &sample.location;
        let coordinate = json!({
            "type":"subresource", "namespace":location.namespace, "field":location.field,
            "indices":location.indices.map(|value| value.to_string()),
        });
        require(
            usage.get("location") == Some(&coordinate)
                && usage.get("scope") == Some(&json!("payload"))
                && original_entry(source, 0, sample.entry.path()),
            "original material name source use differs from checked bytes",
        )
    }

    fn directory(&self, refs: &[String], index: usize, path: &str) -> TestResult {
        let mut found = false;
        for reference in refs {
            let (usage, source) = self.witness(reference)?;
            if !original_entry(source, index, path) {
                continue;
            }
            require(
                usage.get("scope") == Some(&json!("archiveDirectoryMetadata")),
                "target must contribute directory evidence only",
            )?;
            found = true;
        }
        require(
            found,
            "resolved field did not carry the target directory witness",
        )
    }

    fn marker(&self, field: &Field) -> TestResult {
        let mut encrypted = false;
        let mut unknown = false;
        for reference in &field.uses {
            let (usage, _) = self.witness(reference)?;
            match usage.get("effectiveEncryption").and_then(Value::as_str) {
                Some("encrypted") => encrypted = true,
                Some("unknown") => unknown = true,
                Some("unencrypted") => {}
                _ => return Err("invalid evidence encryption state".into()),
            }
        }
        let expected = if encrypted {
            Some(true)
        } else if unknown {
            None
        } else {
            Some(false)
        };
        require(
            field.encryption == expected,
            "resolved marker differs from its admitted source uses",
        )
    }

    fn witness(&self, reference: &str) -> TestResult<(&Value, &Value)> {
        let usage = self
            .uses
            .get(reference)
            .ok_or("source use reference closes")?;
        let source = usage
            .get("source")
            .and_then(Value::as_str)
            .ok_or("use source")?;
        let source = self
            .sources
            .get(source)
            .ok_or("physical source reference closes")?;
        Ok((usage, source))
    }
}

fn original_entry(source: &Value, index: usize, path: &str) -> bool {
    source.get("entryIndex") == Some(&json!(index))
        && source.pointer("/path/value") == Some(&json!(path))
}

fn indexed(mut lines: Lines) -> TestResult<BTreeMap<String, Value>> {
    let mut rows = BTreeMap::new();
    while let Some(row) = lines.next::<Value>()? {
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .ok_or("indexed provenance row ID")?;
        require(
            rows.insert(id.to_owned(), row).is_none(),
            "duplicates in provenance inventory",
        )?;
    }
    Ok(rows)
}

struct Field {
    encryption: Option<bool>,
    uses: Vec<String>,
}

fn field(base: &Path, ordinal: usize, pointer: &str) -> TestResult<Field> {
    let mut lines = Lines::open(&base.join("field-origins.jsonl"))?;
    let ordinal = ordinal.to_string();
    while let Some(field) = lines.next::<Value>()? {
        if field.get("field").and_then(Value::as_str) != Some(pointer)
            || field.pointer("/scope/ordinal").and_then(Value::as_str) != Some(ordinal.as_str())
            || field.pointer("/scope/stream/kind").and_then(Value::as_str) != Some("model")
        {
            continue;
        }
        require(
            field.get("sourceSetComplete") == Some(&json!(true)),
            "individual field needs a complete source set",
        )?;
        let encryption = match field.get("containsEncryptedSource") {
            Some(Value::Null) => None,
            Some(Value::Bool(value)) => Some(*value),
            _ => return Err("missing direct encryption marker".into()),
        };
        let refs = field
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("direct source uses")?;
        let uses = refs
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "source use reference text".into())
            })
            .collect::<TestResult<_>>()?;
        return Ok(Field { encryption, uses });
    }
    Err("no matching field-level provenance".into())
}
