// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::path::Path;

use p4kx_extraction::{
    Archive, PhysicalSource, PhysicalSourceRef, SourceInventory, SourceUseDescription,
};
use serde_json::{Value, json};

use crate::catalog_read::original::with_entry_capture;
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_table::DatasetAvailability;
use crate::starmap_index::Index;
use crate::starmap_parse::installed;
use crate::starmap_parse::{Body, DescriptorScope};
use crate::starmap_walk::SystemRead;
use crate::{CatalogError, ExportAccountingError};

use super::super::LocationPhysicsDescriptorSnapshot;
use super::super::stage;
use super::TestResult;

pub(super) fn compare(
    archive: &Archive,
    outer: u32,
    sdk: &LocationPhysicsDescriptorSnapshot,
    scratch: &Path,
) -> TestResult {
    let produced = project(archive, usize::try_from(outer)?, scratch)
        .map_err(|_| "selected descriptor could not enter the Catalog production stage")?;
    let descriptor = sdk
        .descriptor
        .as_ref()
        .and_then(p4kx_extraction::SourcedValue::value)
        .ok_or("SDK descriptor identity is missing")?;
    if &produced.descriptor != descriptor.input_ref() {
        return Err("staged producer and SDK selected different original descriptors".into());
    }
    let staged = serde_json::from_slice::<Value>(&serde_json::to_vec(&produced.table)?)?;
    let availability = if sdk.grid.is_some() {
        DatasetAvailability::Available
    } else {
        DatasetAvailability::Absent
    };
    let rows = sdk.grid.as_deref().into_iter().collect::<Vec<_>>();
    let expected = json!({"availability": availability, "rows": serde_json::to_value(rows)?});
    if staged != expected {
        return Err("staged table and SDK differ in wire rows or scoped availability".into());
    }
    let staged_evidence = Evidence::new(&produced.inventory);
    let sdk_evidence = Evidence::new(&sdk.source_inventory);
    source_closure(
        &staged,
        &produced.descriptor,
        &staged_evidence,
        &sdk_evidence,
    )
}

struct Produced {
    table: StagedCatalogTable,
    descriptor: PhysicalSourceRef,
    inventory: SourceInventory,
}

fn project(archive: &Archive, entry: usize, scratch: &Path) -> Result<Produced, CatalogError> {
    let index = Index::new(archive);
    with_entry_capture(archive, |capture| {
        let scope = DescriptorScope::Catalog {
            jump_endpoints: false,
        };
        let Body::Parsed { file, .. } =
            installed::read_witness_body(archive, &index, entry, scope, capture)?
        else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let selected = file.descriptor_selector()?;
        let mut system = SystemRead::default();
        system
            .location_physics
            .record(file.reference(), &file.location_physics)?;
        let grid = stage::write(&[system], scratch, None)?;
        Ok(Produced {
            table: StagedCatalogTable::new(grid.availability, grid.spool, None),
            descriptor: selected.description().source().clone(),
            inventory: capture.source_uses().freeze()?,
        })
    })
}

struct Evidence<'a> {
    sources: HashMap<&'a str, &'a PhysicalSource>,
    uses: HashMap<&'a str, &'a SourceUseDescription>,
}

impl<'a> Evidence<'a> {
    fn new(inventory: &'a SourceInventory) -> Self {
        Self {
            sources: inventory
                .iter_sources()
                .map(|source| (source.id().as_str(), source))
                .collect(),
            uses: inventory
                .iter_uses()
                .map(|usage| (usage.id().as_str(), usage))
                .collect(),
        }
    }
}

fn source_closure<'a>(
    table: &'a Value,
    descriptor: &'a PhysicalSourceRef,
    producer: &Evidence<'a>,
    consumer: &Evidence<'_>,
) -> TestResult {
    let mut sources = vec![descriptor.as_str()];
    let mut pending = vec![table];
    while let Some(value) = pending.pop() {
        if let Some(object) = value.as_object() {
            if let Some(reference) = object.get("inputRef") {
                sources.push(reference.as_str().ok_or("invalid row input reference")?);
            }
            if let Some(references) = object.get("sourceUsesRefs") {
                for reference in references.as_array().ok_or("invalid source-use list")? {
                    let key = reference.as_str().ok_or("invalid source-use reference")?;
                    let usage = producer
                        .uses
                        .get(key)
                        .copied()
                        .ok_or("missing staged source use")?;
                    if Some(usage) != consumer.uses.get(key).copied() {
                        return Err("staged table and SDK differ in a physical source use".into());
                    }
                    sources.push(usage.source().as_str());
                }
            }
            pending.extend(object.values());
        } else if let Some(array) = value.as_array() {
            pending.extend(array);
        }
    }
    compare_sources(sources, producer, consumer)
}

fn compare_sources<'a>(
    mut pending: Vec<&'a str>,
    producer: &Evidence<'a>,
    consumer: &Evidence<'_>,
) -> TestResult {
    let mut visited = HashSet::new();
    while let Some(reference) = pending.pop() {
        if !visited.insert(reference) {
            continue;
        }
        let source = producer
            .sources
            .get(reference)
            .copied()
            .ok_or("missing staged physical source")?;
        if Some(source) != consumer.sources.get(reference).copied() {
            return Err("staged table and SDK differ in a physical source or ancestor".into());
        }
        pending.extend(source.ancestors().iter().map(PhysicalSourceRef::as_str));
    }
    Ok(())
}
