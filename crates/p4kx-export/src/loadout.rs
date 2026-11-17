// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities, LoadoutResolver};
use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, Value};

use crate::catalog_rows::spool::{RowSpoolError, checkpoint};
use crate::catalog_v3::catalog_item_alignment::{AlignmentIndex, ItemAlignmentRows};
use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort};
use crate::guid_text::{self, CatalogGuid};
use crate::loadout_port::PortReader;
use crate::record_fields::elements;
use crate::{CatalogError, CatalogReadFailure, LoadoutSummary};

#[path = "loadout_alignment.rs"]
pub(crate) mod alignment;
#[path = "loadout_ledger.rs"]
pub(crate) mod ledger;

use crate::Kind;
use crate::export_ledger::ExportLedger;
use ledger::Capture;
use staging::{StagedLoadoutWriter, StagedLoadouts};

#[path = "staged_loadouts.rs"]
pub(crate) mod staging;

const DEFAULT_LOADOUT: &str = "SEntityComponentDefaultLoadoutParams";
const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";

/// Returns whether the Record tree `tree` may have ports: its Record defines item ports or names a
/// default Loadout. Only such an item is resolved.
pub(crate) fn carries_ports(tree: &RecordTree<'_>) -> bool {
    let Some(Value::Array(components)) = tree.field("Components") else {
        return false;
    };
    components.iter().any(|component| match component {
        Value::Struct(component) => match component.type_name() {
            DEFAULT_LOADOUT => true,
            PORT_CONTAINER => !elements(Some(component), "Ports").is_empty(),
            _ => false,
        },
        _ => false,
    })
}

/// An entry of `ships` or `items` whose ports are wanted.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wanted {
    pub(crate) record_index: u32,
    pub(crate) guid: CatalogGuid,
    pub(crate) kind: EntityKind,
}

/// The loadouts of a catalog, the Records that could not be resolved and what was counted.
pub(crate) struct Loadouts<'f> {
    pub(crate) entries: StagedLoadouts,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) summary: LoadoutSummary,
    pub(crate) ledger: ExportLedger,
}

#[derive(Clone, Copy)]
pub(crate) struct SelectedSources<'rows, 'source> {
    pub(crate) ledger: &'rows ExportLedger,
    pub(crate) alignment: &'rows ItemAlignmentRows<'source>,
}

/// Resolves the Default Loadout of every entity in `wanted` and returns the ports of each,
/// ordered by GUID. The first resolver request in that order owns each cold read.
///
/// A ship always gets a loadout; an item gets one only when its tree holds a port. The tree stops
/// at docked vehicles (`LoadoutResolver::stopping_at_docked_vehicles`). `known` holds the GUIDs of
/// the entries of `items` and `ships`; the attached GUIDs outside it are counted in the summary.
///
/// # Errors
///
/// Returns an error if resolution accounting fails, staging cannot write, or cancellation is requested.
pub(crate) fn loadouts<'f>(
    files: &ArchiveFiles<'_>,
    entities: &Arc<Entities<'f>>,
    wanted: impl Iterator<Item = Wanted>,
    known: &HashSet<Guid>,
    sources: SelectedSources<'_, 'f>,
    input: &crate::build::original_input::OriginalCatalogInput<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<Loadouts<'f>, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let capture = Arc::new(Capture::for_input(
        input,
        sources.ledger,
        cancel,
        Kind::Loadouts,
    ));
    let alignment = AlignmentIndex::new(sources.alignment)?;
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::clone(entities), capture.clone())
            .with_files(files)
            .stopping_at_docked_vehicles();
    let mut reader = PortReader::with_alignment(&alignment);
    let scratch = std::env::temp_dir();
    let mut entries = StagedLoadoutWriter::new(&scratch, cancel);
    let mut summary = LoadoutSummary::default();
    let mut outside = HashSet::new();
    let failures = resolve_ordered(
        wanted,
        cancel,
        |want| read_with(&mut resolver, &mut reader, entities.forge(), want, &capture),
        |entry| {
            summary.add_entry(&entry, known, &mut outside);
            entries.append(entry)
        },
    );
    drop(resolver);
    let mut ledger = capture.finish()?;
    drop(capture);
    let failures = failures?;
    ledger.inventory(Kind::Loadouts);
    summary.outside = outside.len();
    Ok(Loadouts {
        entries: entries.finish()?,
        failures,
        summary,
        ledger,
    })
}

fn resolve_ordered<'source>(
    wanted: impl Iterator<Item = Wanted>,
    cancel: Option<&CancelToken>,
    mut read: impl FnMut(Wanted) -> Result<Option<Loadout>, CatalogReadFailure<'source>>,
    mut accept: impl FnMut(Loadout) -> Result<(), RowSpoolError>,
) -> Result<Vec<CatalogReadFailure<'source>>, CatalogError> {
    let mut ordered: Vec<_> = wanted.enumerate().collect();
    guid_text::sort_by_guid(&mut ordered, |(_, want)| want.guid);
    let mut failures = Vec::new();
    for (position, want) in ordered {
        checkpoint(cancel)?;
        match read(want) {
            Ok(Some(entry)) => accept(entry)?,
            Ok(None) => {}
            Err(failure) => failures.push((position, failure)),
        }
    }
    // Equal-GUID failures are deduplicated in input order by the collector.
    failures.sort_unstable_by_key(|(position, _)| *position);
    Ok(failures.into_iter().map(|(_, failure)| failure).collect())
}

fn read_with<'f: 'worker, 'worker>(
    resolver: &mut LoadoutResolver<'worker>,
    reader: &mut PortReader<'_>,
    forge: &'f DataForge<'f>,
    want: Wanted,
    capture: &Capture<'_>,
) -> Result<Option<Loadout>, CatalogReadFailure<'f>> {
    let Some(record) = forge
        .record(want.record_index)
        .filter(|record| record.guid() == want.guid.0)
    else {
        capture.missing_root();
        return Ok(None);
    };
    let node = match resolver.resolve(&record) {
        Ok(node) => node,
        Err(error) => {
            capture.root(&record, want.kind, Err(&error));
            return Err(CatalogReadFailure::of_loadout(&record, &error));
        }
    };
    capture.root(&record, want.kind, Ok(&node));
    let ports = match reader.ports(&node, capture) {
        Ok(ports) => ports,
        Err(error) => {
            capture.record_error(error);
            return Ok(None);
        }
    };
    let issues = reader.take_issues();
    if want.kind == EntityKind::Item && ports.is_empty() {
        return Ok(None);
    }
    Ok(Some(Loadout {
        root_record_index: want.record_index,
        port_inventory_complete: node.issues().is_empty(),
        entity_guid: want.guid,
        entity_kind: want.kind,
        issues,
        ports,
    }))
}

#[cfg(test)]
#[path = "loadout_order_tests.rs"]
mod order_tests;

impl LoadoutSummary {
    fn add_entry(&mut self, entry: &Loadout, known: &HashSet<Guid>, outside: &mut HashSet<Guid>) {
        match entry.entity_kind {
            EntityKind::Ship => self.ships += 1,
            EntityKind::Item => self.items += 1,
        }
        if !entry.issues.is_empty() {
            self.with_issues += 1;
        }
        self.count(&entry.ports, known, outside);
    }

    fn count(&mut self, ports: &[LoadoutPort], known: &HashSet<Guid>, outside: &mut HashSet<Guid>) {
        for port in ports {
            self.ports = self.ports.saturating_add(1);
            if port.unresolved.is_some() {
                self.unresolved = self.unresolved.saturating_add(1);
            } else if let Some(guid) = port.item_guid {
                self.attached = self.attached.saturating_add(1);
                if !known.contains(&guid.0) {
                    outside.insert(guid.0);
                }
            }
            if port.docked_vehicle {
                self.docked_vehicles = self.docked_vehicles.saturating_add(1);
            }
            self.count(&port.ports, known, outside);
        }
    }
}
