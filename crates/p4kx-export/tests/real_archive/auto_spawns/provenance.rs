// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{DataForgeOrigin, InputScope, SourceEncryption, SourceInput, SourceUses};
use serde_json::{Value, json};

use super::TestResult;
use crate::catalog_v3_consumer;
use crate::real_archive_contract_templates::mission_graph as graph;

pub(super) struct Origins<'catalog> {
    input: String,
    marker: Value,
    uses: HashMap<&'catalog str, &'catalog Value>,
}

impl<'catalog> Origins<'catalog> {
    pub(super) fn read(catalog: &'catalog Value, origin: &DataForgeOrigin) -> TestResult<Self> {
        let DataForgeOrigin::Archive(entry) = origin else {
            return Err("original DataForge must be an Archive Entry".into());
        };
        let sources = SourceUses::new();
        let scope = sources.observe(SourceInput::Archive(entry), InputScope::Payload)?;
        let input = scope.source_ref().as_str().to_owned();
        let original = serde_json::to_value(sources.freeze()?)?;
        let authored = graph::array(&original, "sources")?;
        let physical = catalog
            .pointer("/provenance/sources")
            .and_then(Value::as_array)
            .ok_or("physical source inventory")?;
        assert!(
            authored.iter().all(|source| physical.contains(source)),
            "the original DataForge source and its physical parents are present"
        );
        let marker = match origin.encryption() {
            SourceEncryption::Encrypted => Value::Bool(true),
            SourceEncryption::Unencrypted => Value::Bool(false),
            _ => Value::Null,
        };
        let rows = catalog.get("provenance").ok_or("source inventory")?;
        let mut uses = HashMap::new();
        for row in graph::array(rows, "uses")? {
            let id = graph::text(row, "/id")?;
            assert!(uses.insert(id, row).is_none(), "source-use IDs are unique");
        }
        Ok(Self {
            input,
            marker,
            uses,
        })
    }

    pub(super) fn check_owner(&self, row: &Value) -> TestResult {
        assert!(
            graph::text(row, "/inputRef")? == self.input,
            "the physical owner links to the independently observed DataForge Entry"
        );
        Ok(())
    }

    pub(super) fn occurrence(
        &self,
        cell: &Value,
        owner: u32,
        ordinal: usize,
        field: &str,
    ) -> TestResult {
        self.locations(cell, &[value_location(owner, ordinal, field)])
    }

    pub(super) fn reference_field(
        &self,
        cell: &Value,
        owner: u32,
        ordinal: usize,
        target: u32,
        header: Option<&str>,
    ) -> TestResult {
        let caller = value_location(owner, ordinal, "$value");
        let Some(header) = header else {
            return self.locations(cell, &[caller]);
        };
        let target = json!({
            "type": "subresource",
            "namespace": "dataforgeRecordHeader",
            "indices": [target.to_string()],
            "field": header,
        });
        self.locations(cell, &[caller, target])
    }

    fn locations(&self, cell: &Value, expected: &[Value]) -> TestResult {
        let mut seen = vec![false; expected.len()];
        catalog_v3_consumer::source_value(cell)?;
        assert!(
            cell.get("containsEncryptedSource") == Some(&self.marker)
                && cell.get("sourceSetComplete").and_then(Value::as_bool) == Some(true),
            "the datum states its exact Archive encryption and complete source set"
        );
        for reference in graph::array(cell, "sourceUsesRefs")? {
            let use_row = self
                .uses
                .get(reference.as_str().ok_or("source-use reference text")?)
                .ok_or("source-use reference closure")?;
            assert!(
                use_row.get("source").and_then(Value::as_str) == Some(self.input.as_str())
                    && use_row.get("scope").and_then(Value::as_str) == Some("payload")
                    && matches!(
                        use_row.get("role").and_then(Value::as_str),
                        Some("payload" | "selector" | "conversion")
                    ),
                "the datum cites only contributing uses in the admitted DataForge payload"
            );
            for (location, seen) in expected.iter().zip(&mut seen) {
                *seen |= use_row.get("location") == Some(location);
            }
        }
        assert!(
            seen.into_iter().all(|matched| matched),
            "the datum cites each required original coordinate"
        );
        Ok(())
    }
}

fn value_location(owner: u32, ordinal: usize, field: &str) -> Value {
    json!({
        "type": "subresource",
        "namespace": "dataforgeValue",
        "indices": [owner.to_string(), ordinal.to_string()],
        "field": field,
    })
}
