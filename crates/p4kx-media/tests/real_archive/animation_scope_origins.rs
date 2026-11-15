// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use p4kx_extraction::Archive;
use serde_json::Value;

use super::super::archive_replay::Lines;
use super::loadout::Site;
use super::{TestResult, require};

#[derive(Default)]
pub(super) struct Fields<'a>(BTreeMap<usize, BTreeMap<&'static str, &'a BTreeSet<Site>>>);

type SourceFields = BTreeMap<(usize, &'static str), Field>;

impl<'a> Fields<'a> {
    pub(super) fn expect(
        &mut self,
        ordinal: usize,
        field: &'static str,
        sites: &'a BTreeSet<Site>,
    ) {
        if !sites.is_empty() {
            self.0.entry(ordinal).or_default().insert(field, sites);
        }
    }

    pub(super) fn verify(&self, base: &Path, archive: &Archive) -> TestResult {
        if self.0.is_empty() {
            return Ok(());
        }
        let fields = self.read(base)?;
        let sets = memberships(
            base,
            &fields
                .values()
                .filter_map(|field| field.set.as_deref())
                .collect(),
        )?;
        let references = fields
            .values()
            .flat_map(|field| field.uses.iter())
            .chain(sets.values().flatten())
            .map(String::as_str)
            .collect();
        let uses = indexed(base, "origin-uses.jsonl", &references)?;
        let source_ids = uses
            .values()
            .map(|row| text(row, "/source"))
            .collect::<TestResult<BTreeSet<_>>>()?;
        let evidence = Evidence {
            sources: indexed(base, "origin-sources.jsonl", &source_ids)?,
            uses,
            archive,
        };
        for (&ordinal, expected_fields) in &self.0 {
            for (&field, &required) in expected_fields {
                let origin = fields
                    .get(&(ordinal, field))
                    .ok_or("original XML field origin")?;
                let shared = origin
                    .set
                    .as_deref()
                    .map(|set| {
                        sets.get(set)
                            .map(Vec::as_slice)
                            .ok_or("original XML field use set")
                    })
                    .transpose()?
                    .unwrap_or_default();
                evidence.verify(origin, shared, required)?;
            }
        }
        Ok(())
    }

    fn read(&self, base: &Path) -> TestResult<SourceFields> {
        let mut input = Lines::open(&base.join("field-origins.jsonl"))?;
        let mut fields = BTreeMap::new();
        while let Some(row) = input.next::<Value>()? {
            if row.get("type").and_then(Value::as_str) != Some("field")
                || row.pointer("/scope/stream/type").and_then(Value::as_str) != Some("records")
                || row.pointer("/scope/stream/kind").and_then(Value::as_str)
                    != Some("animation-definition")
            {
                continue;
            }
            let ordinal = text(&row, "/scope/ordinal")?.parse()?;
            let field = text(&row, "/field")?;
            if let Some((&field, _)) = self
                .0
                .get(&ordinal)
                .and_then(|expected| expected.get_key_value(field))
            {
                require(
                    fields
                        .insert((ordinal, field), Field::read(&row)?)
                        .is_none(),
                    "exactly one provenance row for each XML-backed field",
                )?;
            }
        }
        Ok(fields)
    }
}

struct Field {
    encrypted: Option<bool>,
    uses: Vec<String>,
    set: Option<String>,
}

impl Field {
    fn read(row: &Value) -> TestResult<Self> {
        let encrypted = match row.get("containsEncryptedSource") {
            Some(Value::Bool(value)) => Some(*value),
            Some(Value::Null) => None,
            _ => return Err("direct field encryption marker".into()),
        };
        let uses = row
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("field original references")?
            .iter()
            .map(|reference| text(reference, "").map(str::to_owned))
            .collect::<TestResult<_>>()?;
        require(
            row.get("sourceSetComplete")
                .and_then(Value::as_bool)
                .is_some(),
            "direct source-set completeness marker",
        )?;
        let set = row
            .get("sourceUseSetRef")
            .map(|reference| text(reference, "").map(str::to_owned))
            .transpose()?;
        Ok(Self {
            encrypted,
            uses,
            set,
        })
    }
}

struct Evidence<'a> {
    uses: BTreeMap<String, Value>,
    sources: BTreeMap<String, Value>,
    archive: &'a Archive,
}

impl Evidence<'_> {
    fn verify(&self, field: &Field, shared: &[String], expected: &BTreeSet<Site>) -> TestResult {
        let mut observed = BTreeSet::new();
        let mut encrypted = false;
        for reference in field.uses.iter().chain(shared) {
            let usage = self.uses.get(reference).ok_or("placed XML source use")?;
            let Some(namespace) = location_namespace(usage) else {
                continue;
            };
            let source = self
                .sources
                .get(text(usage, "/source")?)
                .ok_or("placed XML source")?;
            let position = source
                .get("entryIndex")
                .and_then(Value::as_u64)
                .ok_or("physical XML Entry index")?;
            let entry = self
                .archive
                .entries()
                .get(usize::try_from(position)?)
                .ok_or("original XML Entry")?;
            require(
                text(source, "/path/value")? == entry.path()
                    && text(usage, "/scope")? == "payload"
                    && text(usage, "/role")? == "selector"
                    && text(usage, "/effectiveEncryption")?
                        == if entry.is_encrypted() {
                            "encrypted"
                        } else {
                            "unencrypted"
                        },
                "field cites the verified XML Entry payload as a selector",
            )?;
            encrypted |= entry.is_encrypted();
            observed.insert(Site {
                entry: entry.index(),
                namespace,
                indices: [
                    text(usage, "/location/indices/0")?.parse()?,
                    text(usage, "/location/indices/1")?.parse()?,
                ],
                field: text(usage, "/location/field")?.to_owned(),
            });
        }
        require(
            &observed == expected && (!encrypted || field.encrypted == Some(true)),
            "exact XML attribute sites and their direct encryption marker",
        )
    }
}

fn location_namespace(usage: &Value) -> Option<&'static str> {
    match usage.pointer("/location/namespace").and_then(Value::as_str) {
        Some("loadout-xml-attribute") => Some("loadout-xml-attribute"),
        Some("loadout-cryxml-attribute") => Some("loadout-cryxml-attribute"),
        _ => None,
    }
}

fn memberships<'a>(
    base: &Path,
    wanted: &BTreeSet<&'a str>,
) -> TestResult<BTreeMap<&'a str, Vec<String>>> {
    let mut sets = BTreeMap::new();
    if wanted.is_empty() {
        return Ok(sets);
    }
    let mut input = Lines::open(&base.join("origin-use-sets.jsonl"))?;
    let mut current = None;
    while let Some(row) = input.next::<Value>()? {
        match text(&row, "/type")? {
            "set" => {
                current = wanted.get(text(&row, "/id")?).copied();
                if let Some(id) = current {
                    require(
                        sets.insert(id, Vec::new()).is_none(),
                        "unique shared origin set",
                    )?;
                }
            }
            "member" => {
                if let Some(members) = current.and_then(|id| sets.get_mut(id)) {
                    members.push(text(&row, "/sourceUseRef")?.to_owned());
                }
            }
            "end" => current = None,
            "eof" => break,
            _ => return Err("unrecognised origin use-set row".into()),
        }
    }
    Ok(sets)
}

fn indexed(
    base: &Path,
    stream: &str,
    wanted: &BTreeSet<&str>,
) -> TestResult<BTreeMap<String, Value>> {
    let mut lines = Lines::open(&base.join(stream))?;
    let mut rows = BTreeMap::new();
    while let Some(row) = lines.next::<Value>()? {
        let id = text(&row, "/id")?;
        if wanted.contains(id) {
            require(
                rows.insert(id.to_owned(), row).is_none(),
                "unique origin ID",
            )?;
        }
    }
    Ok(rows)
}

fn text<'a>(row: &'a Value, pointer: &str) -> TestResult<&'a str> {
    row.pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing origin text at {pointer}").into())
}
