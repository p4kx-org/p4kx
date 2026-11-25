// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, Record, TreeBuilder};

use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::contract_loadout::EntityKind;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::loadout::{PortAdmission, Wanted};
use crate::{CatalogError, Kind};

pub(super) struct Scope {
    pub(super) wanted: Vec<Wanted>,
    pub(super) coverage: CatalogRecordCoverage,
}

pub(super) fn read(
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<Scope, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    let mut ships = Vec::new();
    let mut items = Vec::new();
    let mut trees = TreeBuilder::new();
    selection.for_each(forge, cancel, |record| {
        checkpoint(cancel)?;
        let Some(kind) = classify(record, &mut coverage) else {
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
    Ok(Scope {
        wanted: ships,
        coverage,
    })
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
            (classify(&record, &mut coverage) == Some(EntityKind::Ship)).then(|| Wanted {
                record_index: record.index(),
                guid: CatalogGuid(record.guid()),
                kind: EntityKind::Ship,
            })
        })
        .into_iter()
        .collect();
    Ok(Scope { wanted, coverage })
}

fn classify(record: &Record<'_>, coverage: &mut CatalogRecordCoverage) -> Option<EntityKind> {
    let selection = record.type_name().and_then(|kind| {
        if kind == p4kx_assembly::ENTITY_TYPE {
            entity_selection::select(record, None)
        } else {
            Ok(None)
        }
    });
    match selection {
        Ok(Some(Selected::Entity(Kind::Ships))) => Some(EntityKind::Ship),
        Ok(Some(Selected::Entity(Kind::Items))) => Some(EntityKind::Item),
        Ok(Some(Selected::Leftover(Kind::Ships | Kind::Items))) => {
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
