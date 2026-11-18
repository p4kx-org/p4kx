// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};
use p4kx_extraction::{CancelToken, Guid, RecordTree, Value};

use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort};
use crate::record_fields::elements;
use crate::{CatalogError, CatalogReadFailure, LoadoutSummary};

#[path = "loadout_alignment.rs"]
pub(crate) mod alignment;
#[path = "loadout_ledger.rs"]
pub(crate) mod ledger;

use crate::export_ledger::ExportLedger;
#[cfg(test)]
use resolution::read_with;
pub(crate) use resolution::{SelectedSources, Wanted};
use staging::{StagedLoadoutWriter, StagedLoadouts};

#[path = "loadout_resolution.rs"]
pub(crate) mod resolution;

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

/// The loadouts of a catalog, the Records that could not be resolved and what was counted.
pub(crate) struct Loadouts<'f> {
    pub(crate) entries: StagedLoadouts,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) summary: LoadoutSummary,
    pub(crate) ledger: ExportLedger,
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
    let scratch = std::env::temp_dir();
    let mut entries = StagedLoadoutWriter::new(&scratch, cancel);
    let mut summary = LoadoutSummary::default();
    let mut outside = HashSet::new();
    let read = resolution::read_rows(files, entities, wanted, sources, input, cancel, |entry| {
        summary.add_entry(&entry, known, &mut outside);
        entries.append(entry)?;
        Ok(())
    })?;
    summary.outside = outside.len();
    Ok(Loadouts {
        entries: entries.finish()?,
        failures: read.failures,
        summary,
        ledger: read.ledger,
    })
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
