// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::path::Path;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::starmap::ContainerPlacementEvent;
use p4kx_extraction::{SourceInventory, SourceLocation, SourceUseRole};
use serde::Serialize;
use serde_json::Value;

use super::super::digest_parity::{capture_physical_sources, fingerprint};
use super::super::{BoundMeasurement, Counts, TestResult, collect_references, wire_row};
use super::digest_runs::{DigestRuns, Fingerprint, OriginDigest};
use super::wire::{OrderedRows, encounter_key};
use super::{StreamedComponentDigests, spill};

const TABLES: [&str; 2] = [
    "/placements/containerInstances",
    "/placements/unresolvedContainerLinks",
];

pub(crate) struct StreamedPlacementDigests<'directory> {
    directory: &'directory Path,
    comparison: StreamedComponentDigests<'directory, 2>,
    root_use_ids: DigestRuns<'directory>,
    root_source_ids: HashSet<String>,
}

impl<'directory> StreamedPlacementDigests<'directory> {
    pub(crate) fn new(directory: &'directory Path) -> Self {
        Self {
            directory,
            comparison: StreamedComponentDigests {
                rows: std::array::from_fn(|_| OrderedRows::in_encounter_order()),
                uses: DigestRuns::new(directory),
                sources: DigestRuns::new(directory),
            },
            root_use_ids: DigestRuns::new(directory),
            root_source_ids: HashSet::new(),
        }
    }

    pub(crate) fn observe(&mut self, event: &ContainerPlacementEvent<'_>) -> TestResult {
        match event {
            ContainerPlacementEvent::Instance(row) => {
                let [instances, _] = &mut self.comparison.rows;
                retain_row(
                    instances,
                    &mut self.root_use_ids,
                    &mut self.root_source_ids,
                    row,
                )
            }
            ContainerPlacementEvent::UnresolvedLink(row) => {
                let [_, unresolved] = &mut self.comparison.rows;
                retain_row(
                    unresolved,
                    &mut self.root_use_ids,
                    &mut self.root_source_ids,
                    row,
                )
            }
            ContainerPlacementEvent::RootCompleted(root) => {
                self.retain_root_origins(&root.source_inventory)
            }
            _ => Err("unsupported container placement witness event".into()),
        }
    }

    fn retain_root_origins(&mut self, inventory: &SourceInventory) -> TestResult {
        let pending = std::mem::replace(&mut self.root_use_ids, DigestRuns::new(self.directory));
        let wanted = pending.finish()?;
        if wanted.count() == 0 && self.root_source_ids.is_empty() {
            return Ok(());
        }
        let mut sources: HashSet<_> = self.root_source_ids.iter().map(String::as_str).collect();
        let mut cursor = wanted.cursor()?;
        let mut found = 0u64;
        for used in inventory.iter_uses() {
            let id = used.id().as_str();
            if cursor.observe(id)?.is_none() {
                continue;
            }
            if !matches!(
                used.role(),
                SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
            ) || matches!(used.location(), SourceLocation::LocalizationLookup { .. })
            {
                return Err(
                    "the placement row refers to a use outside its original coordinate scope"
                        .into(),
                );
            }
            found = found
                .checked_add(1)
                .ok_or("placement source use counter overflow")?;
            spill(&mut self.comparison.uses, id, serde_json::to_value(used)?)?;
            sources.insert(used.source().as_str());
        }
        if found != wanted.count() {
            return Err(format!(
                "root source use closure is incomplete: expected {}, observed {found}",
                wanted.count()
            )
            .into());
        }
        capture_physical_sources(inventory, sources.into_iter().collect(), |id, source| {
            spill(&mut self.comparison.sources, id, source)
        })?;
        self.root_source_ids.clear();
        Ok(())
    }
}

fn retain_row(
    ordered: &mut OrderedRows,
    use_ids: &mut DigestRuns<'_>,
    source_ids: &mut HashSet<String>,
    row: &impl Serialize,
) -> TestResult {
    let wire = wire_row(row)?;
    let key = encounter_key(&wire)?;
    let mut references = HashSet::new();
    collect_references(&wire, &mut references);
    if references.is_empty() {
        return Err("a canonical placement row lacks contributing sources".into());
    }
    for reference in references {
        use_ids.push(OriginDigest {
            id: reference.into(),
            payload: Fingerprint::default(),
        })?;
    }
    collect_input_refs(&wire, source_ids)?;
    ordered.append_encounter_row(key, fingerprint(wire)?)
}

fn collect_input_refs(value: &Value, sources: &mut HashSet<String>) -> TestResult {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_input_refs(value, sources)?;
            }
        }
        Value::Object(fields) => {
            for (name, field) in fields {
                if name == "inputRef" {
                    let reference = field.as_str().ok_or("a physical inputRef is not an ID")?;
                    if !sources.contains(reference) {
                        sources.insert(reference.to_owned());
                    }
                } else {
                    collect_input_refs(field, sources)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

impl BoundMeasurement {
    pub(crate) fn verify_streamed_placement_tables(
        &self,
        availability: DatasetAvailability,
        evidence: StreamedPlacementDigests<'_>,
    ) -> TestResult<Counts<2>> {
        if !evidence.root_source_ids.is_empty() || evidence.root_use_ids.finish()?.count() != 0 {
            return Err("the placement stream ended without a complete root inventory".into());
        }
        self.verify_streamed_component_tables(TABLES, [availability; 2], evidence.comparison)
    }
}
