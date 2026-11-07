// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use p4kx_assembly::{ArchiveFiles, Entities, LoadoutResolver};
use p4kx_extraction::{Archive, CancelToken, DataForge, Guid, RecordTree, Value};
use rayon::prelude::*;

use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort};
use crate::guid_text::CatalogGuid;
use crate::loadout_port::PortReader;
use crate::record_fields::elements;
use crate::{CatalogError, CatalogFailure, LoadoutSummary};

#[path = "loadout_ledger.rs"]
pub(crate) mod ledger;

use crate::Kind;
use crate::export_ledger::ExportLedger;
use ledger::Capture;

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

/// The resolvers and port readers not in use. A resolver keeps the Records it has read, so one
/// taken for an entity and given back for the next keeps its caches across the splits of the
/// parallel run.
struct Pool<'f, 'a> {
    entities: Arc<Entities<'f>>,
    files: &'a ArchiveFiles<'a>,
    idle: Mutex<Vec<(LoadoutResolver<'f>, PortReader)>>,
    capture: Arc<Capture>,
}

impl<'f> Pool<'f, 'f> {
    fn read(&self, want: Wanted) -> Result<Option<Loadout>, CatalogFailure> {
        let taken = self
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        let (mut resolver, mut reader) = taken.unwrap_or_else(|| {
            (
                LoadoutResolver::with_observed_entities(
                    Arc::clone(&self.entities),
                    self.capture.clone(),
                )
                .with_files(self.files)
                .stopping_at_docked_vehicles(),
                PortReader::new(),
            )
        });
        let result = read_with(
            &mut resolver,
            &mut reader,
            self.entities.forge(),
            want,
            &self.capture,
        );
        self.idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((resolver, reader));
        result
    }
}

/// An entry of `ships` or `items` whose ports are wanted.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wanted {
    pub(crate) record_index: u32,
    pub(crate) guid: CatalogGuid,
    pub(crate) kind: EntityKind,
}

/// The loadouts of a catalog, the Records that could not be resolved and what was counted.
pub(crate) struct Loadouts {
    pub(crate) entries: Vec<Loadout>,
    pub(crate) failures: Vec<CatalogFailure>,
    pub(crate) summary: LoadoutSummary,
    pub(crate) ledger: ExportLedger,
}

/// Resolves the Default Loadout of every entity in `wanted` and returns the ports of each,
/// ordered by GUID.
///
/// A ship always gets a loadout; an item gets one only when its tree holds a port. The tree stops
/// at docked vehicles (`LoadoutResolver::stopping_at_docked_vehicles`). `known` holds the GUIDs of
/// the entries of `items` and `ships`; the attached GUIDs outside it are counted in the summary.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn loadouts(
    archive: &Archive,
    entities: &Arc<Entities<'_>>,
    wanted: &[Wanted],
    known: &HashSet<Guid>,
    source: &ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Loadouts, CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let capture = Arc::new(Capture::new(source, cancel));
    let files = ArchiveFiles::new(archive);
    let pool = Pool {
        entities: Arc::clone(entities),
        files: &files,
        idle: Mutex::new(Vec::new()),
        capture: Arc::clone(&capture),
    };
    let read: Vec<Option<Result<Option<Loadout>, CatalogFailure>>> = wanted
        .par_iter()
        .map(|want| {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return None;
            }
            Some(pool.read(*want))
        })
        .collect();
    let mut ledger = capture.finish()?;
    ledger.inventory(Kind::Loadouts);
    let mut entries = Vec::with_capacity(read.len());
    let mut failures = Vec::new();
    for result in read {
        match result.ok_or(CatalogError::Cancelled)? {
            Ok(Some(entry)) => entries.push(entry),
            Ok(None) => {}
            Err(failure) => failures.push(failure),
        }
    }
    entries.sort_by_cached_key(|entry| entry.entity_guid.text());
    let summary = LoadoutSummary::of(&entries, known);
    Ok(Loadouts {
        entries,
        failures,
        summary,
        ledger,
    })
}

fn read_with<'f>(
    resolver: &mut LoadoutResolver<'f>,
    reader: &mut PortReader,
    forge: &'f DataForge<'f>,
    want: Wanted,
    capture: &Capture,
) -> Result<Option<Loadout>, CatalogFailure> {
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
            return Err(CatalogFailure::of_loadout(&record, &error));
        }
    };
    capture.root(&record, want.kind, Ok(&node));
    let ports = reader.ports(&node);
    let issues = reader.take_issues();
    if want.kind == EntityKind::Item && ports.is_empty() {
        return Ok(None);
    }
    Ok(Some(Loadout {
        entity_guid: want.guid,
        entity_kind: want.kind,
        issues,
        ports,
    }))
}

impl LoadoutSummary {
    /// Counts the ports of `entries` and the attached GUIDs outside `known`.
    fn of(entries: &[Loadout], known: &HashSet<Guid>) -> Self {
        let mut summary = Self {
            ships: entries
                .iter()
                .filter(|entry| entry.entity_kind == EntityKind::Ship)
                .count(),
            items: entries
                .iter()
                .filter(|entry| entry.entity_kind == EntityKind::Item)
                .count(),
            with_issues: entries
                .iter()
                .filter(|entry| !entry.issues.is_empty())
                .count(),
            ..Self::default()
        };
        let mut outside = HashSet::new();
        for entry in entries {
            summary.count(&entry.ports, known, &mut outside);
        }
        summary.outside = outside.len();
        summary
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
