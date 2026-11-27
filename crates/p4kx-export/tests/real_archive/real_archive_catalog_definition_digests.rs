// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_extraction::SourceInventory;
use serde::Serialize;
use serde_json::Value;

use super::super::digest_parity::{capture_inventory, fingerprint};
use super::super::{
    BoundMeasurement, Counts, ScopedTextProof, TestResult, collect_references, wire_row,
};
use super::digest_runs::{DigestRuns, Fingerprint};
use super::wire::{DefinitionOwner, OrderedRows};
use super::{StreamedComponentDigests, StreamedRowPaths, spill};

pub(crate) struct StreamedDefinitionDigests<'directory, const TABLES: usize> {
    owners: [DefinitionOwner; TABLES],
    available: [Option<DatasetAvailability>; TABLES],
    rows: StreamedComponentDigests<'directory, TABLES>,
    texts: ScopedTextProof,
}

impl<'directory, const TABLES: usize> StreamedDefinitionDigests<'directory, TABLES> {
    pub(crate) fn new(directory: &'directory Path, owners: [DefinitionOwner; TABLES]) -> Self {
        Self {
            available: [None; TABLES],
            rows: StreamedComponentDigests {
                rows: owners.map(OrderedRows::with_definition),
                uses: DigestRuns::new(directory),
                sources: DigestRuns::new(directory),
            },
            owners,
            texts: ScopedTextProof::default(),
        }
    }

    pub(crate) fn retain(
        &mut self,
        table: usize,
        availability: DatasetAvailability,
        rows: &[impl Serialize],
        inventory: &SourceInventory,
    ) -> TestResult {
        self.record_availability(table, availability)?;
        let mut identities = HashSet::new();
        let mut references = HashSet::new();
        for row in rows {
            let id = self.append_row(table, wire_row(row)?, &mut references)?;
            if !identities.insert(id) {
                return Err("the serialized definition family repeats a physical owner".into());
            }
        }
        if !rows.is_empty() && references.is_empty() {
            return Err("the definition table has no contributing source uses".into());
        }
        spill_inventory(&mut self.rows, inventory, references)
    }

    pub(crate) fn retain_visited_row(
        &mut self,
        table: usize,
        row: Value,
        inventory: &SourceInventory,
    ) -> TestResult {
        let mut references = HashSet::new();
        self.append_row(table, row, &mut references)?;
        if references.is_empty() {
            return Err("the visited definition lacks contributing source uses".into());
        }
        spill_inventory(&mut self.rows, inventory, references)
    }

    pub(crate) fn record_availability(
        &mut self,
        table: usize,
        availability: DatasetAvailability,
    ) -> TestResult {
        let state = self
            .available
            .get_mut(table)
            .ok_or("unknown definition slot")?;
        if state.replace(availability).is_some() {
            return Err("a definition family was retained twice".into());
        }
        Ok(())
    }

    fn append_row(
        &mut self,
        table: usize,
        wire: Value,
        references: &mut HashSet<String>,
    ) -> TestResult<Fingerprint> {
        let owner = self
            .owners
            .get(table)
            .ok_or("missing definition identity")?;
        let ordered = self
            .rows
            .rows
            .get_mut(table)
            .ok_or("missing definition rows")?;
        let id = owner.key(&wire)?;
        add_references(&wire, references);
        ordered.append_encounter_row(id, fingerprint(wire)?)?;
        Ok(id)
    }

    pub(crate) fn retain_texts(
        &mut self,
        public: &[impl Serialize],
        protected: &[impl Serialize],
        inventory: &SourceInventory,
    ) -> TestResult {
        self.texts.retain_with(public, protected, |wires| {
            let mut references = HashSet::new();
            for wire in wires {
                add_references(wire, &mut references);
            }
            if references.is_empty() {
                return Err("a selected text domain has no contributing source uses".into());
            }
            spill_inventory(&mut self.rows, inventory, references)
        })
    }

    pub(crate) fn compare(
        self,
        measurement: &BoundMeasurement,
        paths: [&str; TABLES],
    ) -> TestResult<Counts<TABLES>> {
        self.compare_paths(measurement, StreamedRowPaths::Tables(paths))
    }

    pub(crate) fn compare_arrays(
        self,
        measurement: &BoundMeasurement,
        paths: [&str; TABLES],
    ) -> TestResult<Counts<TABLES>> {
        self.compare_paths(measurement, StreamedRowPaths::Arrays(paths))
    }

    fn compare_paths(
        self,
        measurement: &BoundMeasurement,
        paths: StreamedRowPaths<'_, TABLES>,
    ) -> TestResult<Counts<TABLES>> {
        let availability = self
            .available
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or("a definition availability observation is missing")?
            .try_into()
            .map_err(|_| "the definition observation count differs")?;
        measurement.verify_streamed_scope(
            paths,
            availability,
            self.rows,
            Some(&self.texts),
            |_| {},
            |_| Ok(()),
        )
    }
}

fn add_references(wire: &Value, retained: &mut HashSet<String>) {
    let mut references = HashSet::new();
    collect_references(wire, &mut references);
    for reference in references {
        if !retained.contains(reference) {
            retained.insert(reference.to_owned());
        }
    }
}

fn spill_inventory<const TABLES: usize>(
    custody: &mut StreamedComponentDigests<'_, TABLES>,
    inventory: &SourceInventory,
    references: HashSet<String>,
) -> TestResult {
    if references.is_empty() {
        return Ok(());
    }
    capture_inventory(
        inventory,
        references,
        |id, row| spill(&mut custody.uses, id, row),
        |id, row| spill(&mut custody.sources, id, row),
    )
}

#[test]
fn world_definition_stream_checks_every_row_in_guid_order() -> TestResult {
    use serde::de::DeserializeSeed as _;
    use serde_json::json;

    let original = [
        json!({"recordIndex":{"state":"present","value":7},"cell":{"sourceUsesRefs":["a7"]}}),
        json!({"recordIndex":{"state":"present","value":2},"cell":{"sourceUsesRefs":["a2"]}}),
    ];
    let owner = DefinitionOwner::Record;
    let mut ordered = OrderedRows::with_definition(owner);
    for row in &original {
        ordered.append_encounter_row(owner.key(row)?, fingerprint(row.clone())?)?;
    }
    let expected = ordered.finish();
    let [first, second] = original;
    assert_eq!(
        expected.deserialize(&json!([&first, &second]))?,
        json!(2),
        "the authored order can differ from physical Record index order",
    );
    assert!(
        expected.deserialize(&json!([&second, &first])).is_err(),
        "complete row equality requires the canonical order",
    );
    let changed =
        json!({"recordIndex":{"state":"present","value":7},"cell":{"sourceUsesRefs":["changed"]}});
    assert!(
        expected.deserialize(&json!([changed, &second])).is_err(),
        "the comparison requires the original contributors in every row",
    );
    assert!(
        expected.deserialize(&json!([&first])).is_err(),
        "the comparison rejects a partial canonical table",
    );
    Ok(())
}

#[test]
fn commodity_presentation_stream_matches_the_original_header_use() -> TestResult {
    use serde::de::DeserializeSeed as _;
    use serde_json::json;

    let first = json!({"commodityGuid":{"value":"same","sourceUsesRefs":["first"]}});
    let second = json!({"commodityGuid":{"value":"same","sourceUsesRefs":["second"]}});
    let ambiguous = json!({"commodityGuid":{"value":"same","sourceUsesRefs":["first","second"]}});
    let owner = DefinitionOwner::CommodityPresentation;
    let mut ordered = OrderedRows::with_definition(owner);
    for row in [&first, &second] {
        ordered.append_encounter_row(owner.key(row)?, fingerprint(row.clone())?)?;
    }
    let expected = ordered.finish();
    assert_eq!(
        expected.deserialize(&json!([&first, &second]))?,
        json!(2),
        "nominal duplicates with distinct original headers survive",
    );
    assert!(
        expected.deserialize(&json!([&second, &first])).is_err(),
        "the canonical sequence must preserve the original headers",
    );
    assert!(
        expected.deserialize(&json!([&first, &first])).is_err(),
        "one owner cannot stand in for another with the same GUID",
    );
    assert!(
        owner.key(&ambiguous).is_err(),
        "a presentation requires one direct GUID header use",
    );
    assert!(
        owner
            .key(&json!({"commodityGuid":{"value":"same"}}))
            .is_err(),
        "a GUID value alone cannot identify its physical owner",
    );
    Ok(())
}

#[test]
fn retained_definition_text_requires_contributors_for_a_selected_domain() -> TestResult {
    use p4kx_extraction::SourceUses;
    use serde_json::json;

    let directory = tempfile::tempdir()?;
    let mut proof = StreamedDefinitionDigests::new(directory.path(), [DefinitionOwner::Record]);
    let inventory = SourceUses::new().freeze()?;
    let empty = &[] as &[Value];
    proof.retain_texts(empty, empty, &inventory)?;
    let protected = [json!({"id":{"state":"present","value":"authored"}})];
    let Err(error) = proof.retain_texts(empty, &protected, &inventory) else {
        return Err("a selected text domain was accepted without provenance".into());
    };
    assert_eq!(
        error.to_string(),
        "a selected text domain has no contributing source uses",
    );
    Ok(())
}
