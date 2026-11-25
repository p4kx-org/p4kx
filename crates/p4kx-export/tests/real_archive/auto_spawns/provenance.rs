// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{DataForgeOrigin, InputScope, SourceEncryption, SourceInput, SourceUses};
use serde_json::{Value, json};

use super::TestResult;
use crate::catalog_v3_consumer;
use crate::real_archive_contract_templates::mission_graph as graph;

pub(crate) struct Origins<'catalog> {
    input: String,
    marker: Value,
    uses: HashMap<&'catalog str, &'catalog Value>,
}

impl<'catalog> Origins<'catalog> {
    pub(crate) fn read(catalog: &'catalog Value, origin: &DataForgeOrigin) -> TestResult<Self> {
        let DataForgeOrigin::Archive(entry) = origin else {
            return Err("original DataForge must be an Archive Entry".into());
        };
        let sources = SourceUses::with_compact_coordinates();
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

    pub(crate) fn occurrence(
        &self,
        cell: &Value,
        owner: u32,
        ordinal: usize,
        field: &str,
    ) -> TestResult {
        self.locations(cell, &[value_location(owner, ordinal, field)])
    }

    pub(crate) fn reference_field(
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

    /// Resolves a DataForge row's immediate contributors to their original observations.
    pub(crate) fn with_resolved_contributors(&self, row: &Value) -> TestResult<Value> {
        let mut result = row.clone();
        self.resolve(&mut result)?;
        Ok(result)
    }

    fn resolve(&self, row: &mut Value) -> TestResult {
        match row {
            Value::Object(fields) => {
                for (field, value) in fields {
                    if field == "sourceUsesRefs" {
                        *value = Value::Array(self.contributors(value)?);
                    } else {
                        self.resolve(value)?;
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.resolve(value)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn contributors(&self, refs: &Value) -> TestResult<Vec<Value>> {
        let references = refs.as_array().ok_or("immediate contributor references")?;
        let mut distinct = HashSet::new();
        let mut contributors = Vec::new();
        for reference in references {
            let id = reference.as_str().ok_or("original contributor id")?;
            if !distinct.insert(id) {
                return Err("an immediate source set repeats a contributor".into());
            }
            let observation = self
                .uses
                .get(id)
                .ok_or("original contributor is unresolved")?;
            if observation.get("source").and_then(Value::as_str) != Some(self.input.as_str())
                || observation.get("scope").and_then(Value::as_str) != Some("payload")
                || !matches!(
                    observation.get("role").and_then(Value::as_str),
                    Some("payload" | "selector" | "conversion")
                )
                || observation.get("location").is_none()
                || observation.get("effectiveEncryption").is_none()
            {
                return Err(
                    "the contributor does not establish original DataForge evidence".into(),
                );
            }
            let mut semantic = observation
                .as_object()
                .ok_or("original source use object")?
                .clone();
            semantic.remove("id");
            semantic.remove("source");
            contributors.push(Value::Object(semantic));
        }
        contributors.sort_by_cached_key(ToString::to_string);
        Ok(contributors)
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

#[test]
fn origins_match_the_catalogs_compact_physical_inventory() -> TestResult {
    use std::time::UNIX_EPOCH;

    use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
    use p4kx_extraction::{Archive, DataForgeEntry};
    use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestStruct};

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Fixture"));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
    let catalog = serde_json::from_slice(built.json())?;
    Origins::read(&catalog, entry.origin())?;
    Ok(())
}
