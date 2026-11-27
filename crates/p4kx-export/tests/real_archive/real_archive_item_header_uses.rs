// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{SourceLocation, SourceUseRole};
use serde_json::Value;

use super::{TestResult, catalog_v3_consumer};

pub(super) struct HeaderUses<'catalog> {
    uses: HashMap<&'catalog str, &'catalog Value>,
}

impl<'catalog> HeaderUses<'catalog> {
    pub(super) fn read(
        catalog: &'catalog Value,
        rows: impl Iterator<Item = &'catalog Value>,
    ) -> TestResult<Self> {
        let requested: HashSet<_> = rows.map(reference).collect::<Result<_, _>>()?;
        if requested.is_empty() {
            return Ok(Self {
                uses: HashMap::new(),
            });
        }
        let uses = catalog
            .pointer("/provenance/uses")
            .and_then(Value::as_array)
            .ok_or("original source-use inventory")?;
        let mut mapped = HashMap::new();
        let mut sources = HashSet::new();
        for used in uses {
            let Some(id) = used.get("id").and_then(Value::as_str) else {
                continue;
            };
            if requested.contains(id) {
                if mapped.insert(id, used).is_some() {
                    return Err("original Item header use identifiers must be unique".into());
                }
                sources.insert(
                    used.get("source")
                        .and_then(Value::as_str)
                        .ok_or("original Item header source")?,
                );
            }
        }
        if sources.len() > 1 {
            return Err("physical Item indices belong to one original DataForge input".into());
        }
        let physical = catalog
            .pointer("/provenance/sources")
            .and_then(Value::as_array)
            .ok_or("original physical source inventory")?;
        for source in sources {
            if !physical
                .iter()
                .any(|row| row.get("id").and_then(Value::as_str) == Some(source))
            {
                return Err("original Item header input must resolve in this inventory".into());
            }
        }
        Ok(Self { uses: mapped })
    }

    pub(super) fn key(&self, row: &Value) -> TestResult<(&'catalog str, u32)> {
        let index = catalog_v3_consumer::field(row, p4kx_export::Kind::Items, "recordIndex")?
            .as_u64()
            .ok_or("original physical Item Record")?;
        let index = u32::try_from(index)?;
        let used = self
            .uses
            .get(reference(row)?)
            .copied()
            .ok_or("original physical Item header reference closure")?;
        let location = serde_json::to_value(SourceLocation::Subresource {
            namespace: "dataforgeRecordHeader".to_owned(),
            indices: vec![u64::from(index)],
            field: "recordIndex".to_owned(),
        })?;
        if used.get("location") != Some(&location)
            || used.get("role") != Some(&serde_json::to_value(SourceUseRole::Selector)?)
            || used.get("scope").and_then(Value::as_str) != Some("payload")
        {
            return Err("the physical Item index must bind its original Record header".into());
        }
        let source = used
            .get("source")
            .and_then(Value::as_str)
            .ok_or("original Item header physical source")?;
        Ok((source, index))
    }
}

fn reference(row: &Value) -> TestResult<&str> {
    let carrier = row
        .get("recordIndex")
        .ok_or("original physical Item index carrier")?;
    catalog_v3_consumer::present(carrier)?;
    if carrier.get("sourceSetComplete").and_then(Value::as_bool) != Some(true) {
        return Err("physical Item index evidence must be complete".into());
    }
    let [reference] = carrier
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("original physical Item index uses")?
        .as_slice()
    else {
        return Err("physical Item index requires its one original header use".into());
    };
    reference
        .as_str()
        .ok_or_else(|| "original Item header use identity".into())
}
