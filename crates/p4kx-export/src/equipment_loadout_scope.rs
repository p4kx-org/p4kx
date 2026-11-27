// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, Record, RecordTree, TreeBuilder};

use crate::catalog_queries::record_selection::RecordVisit;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::contract_loadout::EntityKind;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::loadout::{PortAdmission, Wanted};
use crate::{CatalogError, Kind};

/// Selects the physical roots of a resolved Loadout visit.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum LoadoutOwnerSelection<'records> {
    /// Selects Ship and Item roots by the ordinary Catalog Record policy.
    Records(CatalogRecordSelection),
    /// Selects all admitted Ship roots and their reached dependencies.
    Ships,
    /// Selects each supplied physical Record index once.
    RecordIndices(&'records [u32]),
}

pub(super) struct Scope {
    pub(super) wanted: Vec<Wanted>,
    pub(super) coverage: CatalogRecordCoverage,
}

pub(super) struct VisitScope {
    pub(super) owners: Scope,
    pub(super) missing_record_indices: Vec<u32>,
}

#[derive(Clone, Copy)]
enum RootKinds {
    ShipsAndItems,
    Ships,
}

impl RootKinds {
    const fn entity_kind(self, kind: Kind) -> Option<EntityKind> {
        match kind {
            Kind::Ships => Some(EntityKind::Ship),
            Kind::Items if matches!(self, Self::ShipsAndItems) => Some(EntityKind::Item),
            _ => None,
        }
    }
}

pub(super) fn read(
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<Scope, CatalogError> {
    Ok(read_visit(
        forge,
        LoadoutOwnerSelection::Records(selection),
        cancel,
        drop,
    )?
    .owners)
}

pub(super) fn read_visit<'source>(
    forge: &'source DataForge<'source>,
    selection: LoadoutOwnerSelection<'_>,
    cancel: Option<&CancelToken>,
    mut retain_item_tree: impl FnMut(RecordTree<'source>),
) -> Result<VisitScope, CatalogError> {
    let kinds = match selection {
        LoadoutOwnerSelection::Ships => RootKinds::Ships,
        _ => RootKinds::ShipsAndItems,
    };
    let mut coverage = CatalogRecordCoverage::default();
    let mut ships = Vec::new();
    let mut items = Vec::new();
    let mut trees = TreeBuilder::new();
    let missing_record_indices = visit_records(forge, selection, cancel, |record| {
        checkpoint(cancel)?;
        let Some(kind) = classify(record, kinds, &mut coverage) else {
            return Ok(());
        };
        let wanted = Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind,
        };
        match kind {
            EntityKind::Ship => ships.push(wanted),
            EntityKind::Item => match trees.build(record) {
                Ok(tree) => {
                    let admission = PortAdmission::read(&tree);
                    if admission.wanted {
                        items.push(wanted);
                        retain_item_tree(tree);
                    }
                    if admission.incomplete {
                        coverage.incomplete_fields.push(record.index());
                    }
                }
                Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
                Err(_) => coverage.unreadable_bodies.push(record.index()),
            },
        }
        Ok(())
    })?;
    ships.extend(items);
    Ok(VisitScope {
        owners: Scope {
            wanted: ships,
            coverage,
        },
        missing_record_indices,
    })
}

fn visit_records<'source>(
    forge: &'source DataForge<'_>,
    selection: LoadoutOwnerSelection<'_>,
    cancel: Option<&CancelToken>,
    visit: impl FnMut(&Record<'source>) -> Result<(), CatalogError>,
) -> Result<Vec<u32>, CatalogError> {
    let mut missing = Vec::new();
    let records = match selection {
        LoadoutOwnerSelection::Records(selection) => selection.into(),
        LoadoutOwnerSelection::Ships => CatalogRecordSelection::All.into(),
        LoadoutOwnerSelection::RecordIndices(indices) => RecordVisit::Indices {
            indices,
            missing: &mut missing,
        },
    };
    records.try_for_each(forge, cancel, visit)?;
    Ok(missing)
}

pub(super) fn finish_coverage(coverage: &mut CatalogRecordCoverage) {
    coverage.incomplete_fields.sort_unstable();
    coverage.incomplete_fields.dedup();
}

pub(super) fn read_ship(
    forge: &DataForge<'_>,
    index: u32,
    cancel: Option<&CancelToken>,
) -> Result<Scope, CatalogError> {
    checkpoint(cancel)?;
    let mut coverage = CatalogRecordCoverage::default();
    let wanted = forge
        .record(index)
        .and_then(|record| {
            (classify(&record, RootKinds::ShipsAndItems, &mut coverage) == Some(EntityKind::Ship))
                .then(|| Wanted {
                    record_index: record.index(),
                    guid: CatalogGuid(record.guid()),
                    kind: EntityKind::Ship,
                })
        })
        .into_iter()
        .collect();
    Ok(Scope { wanted, coverage })
}

fn classify(
    record: &Record<'_>,
    kinds: RootKinds,
    coverage: &mut CatalogRecordCoverage,
) -> Option<EntityKind> {
    let selection = record.type_name().and_then(|kind| {
        if kind == p4kx_assembly::ENTITY_TYPE {
            entity_selection::select(record, None)
        } else {
            Ok(None)
        }
    });
    match selection {
        Ok(Some(Selected::Entity(kind))) => kinds.entity_kind(kind),
        Ok(Some(Selected::Leftover(kind))) if kinds.entity_kind(kind).is_some() => {
            coverage.skipped_leftovers += 1;
            None
        }
        Err(_) => {
            coverage.unreadable_headers.push(record.index());
            None
        }
        Ok(_) => None,
    }
}

#[cfg(test)]
#[path = "equipment_loadout_scope_tests.rs"]
mod tests;
