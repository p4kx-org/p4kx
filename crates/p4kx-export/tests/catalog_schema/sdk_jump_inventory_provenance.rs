// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::SourceInventory;
use serde_json::Value;

use super::TestResult;

struct Inventory<'rows> {
    uses: BTreeMap<&'rows str, &'rows Value>,
    sources: BTreeMap<&'rows str, &'rows Value>,
}

impl<'rows> Inventory<'rows> {
    fn read(value: &'rows Value) -> TestResult<Self> {
        Ok(Self {
            uses: index(value, "uses")?,
            sources: index(value, "sources")?,
        })
    }
}

pub(super) fn verify(
    catalog: &Value,
    endpoints: &Value,
    inventory: &SourceInventory,
) -> TestResult<()> {
    let canonical = Inventory::read(catalog.get("provenance").ok_or("producer provenance")?)?;
    let scoped = serde_json::to_value(inventory)?;
    let scoped = Inventory::read(&scoped)?;
    let mut references = References::default();
    references.visit(endpoints)?;
    let mut sources: Vec<_> = references.sources.into_iter().collect();
    for id in references.uses {
        let observed = scoped
            .uses
            .get(id)
            .ok_or("endpoint use closes in the scoped inventory")?;
        let expected = canonical
            .uses
            .get(id)
            .ok_or("endpoint use closes in the producer inventory")?;
        assert_eq!(
            observed, expected,
            "the source coordinates, role and encryption of {id} match"
        );
        sources.push(
            observed
                .get("source")
                .and_then(Value::as_str)
                .ok_or("physical source ID")?,
        );
    }
    let mut seen = BTreeSet::new();
    while let Some(id) = sources.pop() {
        if !seen.insert(id) {
            continue;
        }
        let observed = scoped
            .sources
            .get(id)
            .ok_or("physical endpoint source closes in the scoped inventory")?;
        let expected = canonical
            .sources
            .get(id)
            .ok_or("physical endpoint source closes in the producer inventory")?;
        assert_eq!(
            observed, expected,
            "the physical source and ancestry of {id} match"
        );
        let ancestors = observed
            .get("ancestors")
            .and_then(Value::as_array)
            .ok_or("original physical ancestry")?;
        for ancestor in ancestors {
            sources.push(ancestor.as_str().ok_or("ancestor source ID")?);
        }
    }
    Ok(())
}

fn index<'value>(
    inventory: &'value Value,
    name: &str,
) -> TestResult<BTreeMap<&'value str, &'value Value>> {
    let rows = inventory
        .get(name)
        .and_then(Value::as_array)
        .ok_or("source inventory rows")?;
    let mut index = BTreeMap::new();
    for row in rows {
        let id = row
            .get("id")
            .and_then(Value::as_str)
            .ok_or("source inventory row ID")?;
        assert!(index.insert(id, row).is_none(), "each {name} ID is unique");
    }
    Ok(index)
}

#[derive(Default)]
struct References<'rows> {
    uses: BTreeSet<&'rows str>,
    sources: BTreeSet<&'rows str>,
}

impl<'rows> References<'rows> {
    fn visit(&mut self, value: &'rows Value) -> TestResult<()> {
        if let Some(uses) = value.get("sourceUsesRefs").and_then(Value::as_array) {
            for used in uses {
                self.uses
                    .insert(used.as_str().ok_or("field source-use ID")?);
            }
        }
        if let Some(input) = value.get("inputRef").and_then(Value::as_str) {
            self.sources.insert(input);
        }
        match value {
            Value::Object(fields) => {
                for child in fields.values() {
                    self.visit(child)?;
                }
            }
            Value::Array(children) => {
                for child in children {
                    self.visit(child)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
