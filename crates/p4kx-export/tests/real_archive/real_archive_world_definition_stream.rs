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
use super::digest_runs::DigestRuns;
use super::wire::{DefinitionOwner, OrderedRows};
use super::{StreamedComponentDigests, spill};

pub(crate) struct WorldDefinitionDigests<'directory, const TABLES: usize> {
    owners: [DefinitionOwner; TABLES],
    available: [Option<DatasetAvailability>; TABLES],
    rows: StreamedComponentDigests<'directory, TABLES>,
    texts: ScopedTextProof,
}

impl<'directory, const TABLES: usize> WorldDefinitionDigests<'directory, TABLES> {
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
        let state = self
            .available
            .get_mut(table)
            .ok_or("unknown World definition slot")?;
        if state.replace(availability).is_some() {
            return Err("a World definition family was retained twice".into());
        }
        let owner = *self
            .owners
            .get(table)
            .ok_or("missing World definition identity")?;
        let ordered = self
            .rows
            .rows
            .get_mut(table)
            .ok_or("missing World definition rows")?;
        let mut identities = HashSet::new();
        let mut references = HashSet::new();
        for row in rows {
            let wire = wire_row(row)?;
            let id = owner.physical_index(&wire)?;
            if !identities.insert(id) {
                return Err("the serialized World family repeats a physical owner".into());
            }
            add_references(&wire, &mut references);
            ordered.append_definition(id, fingerprint(wire)?)?;
        }
        if !rows.is_empty() && references.is_empty() {
            return Err("the World definition table has no contributing source uses".into());
        }
        spill_inventory(&mut self.rows, inventory, references)
    }

    pub(crate) fn retain_texts(
        &mut self,
        public: &[impl Serialize],
        protected: &[impl Serialize],
        inventory: &SourceInventory,
    ) -> TestResult {
        let mut references = HashSet::new();
        self.texts.retain_with(public, protected, |wires| {
            for wire in wires {
                add_references(wire, &mut references);
            }
            Ok(())
        })?;
        spill_inventory(&mut self.rows, inventory, references)
    }

    pub(crate) fn compare(
        self,
        measurement: &BoundMeasurement,
        paths: [&str; TABLES],
    ) -> TestResult<Counts<TABLES>> {
        let availability = self
            .available
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or("a World definition availability observation is missing")?
            .try_into()
            .map_err(|_| "the World definition observation count differs")?;
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
        |id, row| spill(&mut custody.uses, &id, row),
        |id, row| spill(&mut custody.sources, &id, row),
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
        ordered.append_definition(owner.physical_index(row)?, fingerprint(row.clone())?)?;
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
