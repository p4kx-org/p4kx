// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, DataForge, DataForgeError, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::ammo_definition::{
    self, AmmoDefinition, AmmoSelection, PreparedAmmoDefinition,
};
use crate::catalog_v3::catalog_item_components;
use crate::catalog_v3::catalog_weapon_actions::WeaponActions;
use crate::catalog_v3::catalog_weapon_projectile_ammo::{self, WeaponProjectileAmmoSelection};
use crate::catalog_v3::weapon_combat_base::countermeasure_emitter::{self, CountermeasureEmitter};
use crate::export_diagnostic::ExportDiagnosticSeverity;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind};

/// Canonical projectile routes and reached ammunition of one physical Item.
#[derive(Debug)]
#[non_exhaustive]
pub struct ProjectileAmmoOwner<'source> {
    /// One outcome per authored projectile action occurrence in Catalog order.
    pub selections: Vec<WeaponProjectileAmmoSelection>,
    /// Readable selected Ammo definitions in Catalog order, deduplicated by physical index.
    pub ammo_definitions: Vec<AmmoDefinition<'source>>,
    /// One proved launcher row per distinct original countermeasure role and ammunition.
    pub countermeasure_emitters: Vec<CountermeasureEmitter>,
    /// Body and field coverage of the admitted magazines and Ammo definitions.
    pub dependency_coverage: CatalogRecordCoverage,
}

pub(super) struct ProjectileInputs<'source> {
    pub(super) actions: Vec<WeaponActions<'source>>,
    pub(super) selections: Vec<WeaponProjectileAmmoSelection>,
    pub(super) ammunition: Vec<PreparedAmmoDefinition<'source>>,
    countermeasure_emitters: Vec<CountermeasureEmitter>,
    coverage: CatalogRecordCoverage,
}

impl<'source> ProjectileInputs<'source> {
    pub(super) fn into_owner(self, forge: &DataForge<'_>) -> ProjectileAmmoOwner<'source> {
        ProjectileAmmoOwner {
            selections: catalog_weapon_projectile_ammo::table(forge, self.selections).rows,
            ammo_definitions: self
                .ammunition
                .into_iter()
                .map(PreparedAmmoDefinition::into_definition)
                .collect(),
            countermeasure_emitters: self.countermeasure_emitters,
            dependency_coverage: self.coverage,
        }
    }
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<Vec<ProjectileAmmoOwner<'source>>>, CatalogError> {
    let read = read_inputs(forge, tree, projection, admission, cancel)?;
    Ok(read.map(|inputs| {
        (!inputs.selections.is_empty())
            .then(|| inputs.into_owner(forge))
            .into_iter()
            .collect()
    }))
}

pub(super) fn read_inputs<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<ProjectileInputs<'source>>, CatalogError> {
    let mut dependencies = ExportLedger::default();
    let mut routes = catalog_item_components::read_projectile_ammo(
        forge,
        tree,
        projection,
        admission,
        cancel,
        &mut dependencies,
    )?;
    routes.selection_complete &= routes
        .value
        .selections
        .iter()
        .all(|row| row.selection().origin().source_set_complete());
    let mut coverage = magazine_coverage(dependencies)?;
    let ammunition = reached_ammo(
        forge,
        projection,
        &routes.value.selections,
        cancel,
        &mut coverage,
    )?;
    let countermeasure_emitters = countermeasure_emitter::rows(
        &projection.origins().uses,
        &routes.value.selections,
        &ammunition,
        cancel,
    )?;
    coverage.unreadable_bodies.sort_unstable();
    coverage.unreadable_bodies.dedup();
    coverage.incomplete_fields.sort_unstable();
    coverage.incomplete_fields.dedup();
    Ok(routes.map(|routes| ProjectileInputs {
        actions: routes.actions,
        selections: routes.selections,
        ammunition,
        countermeasure_emitters,
        coverage,
    }))
}

pub(super) fn magazine_coverage(
    ledger: ExportLedger,
) -> Result<CatalogRecordCoverage, CatalogError> {
    let mut coverage = CatalogRecordCoverage::default();
    for row in ledger.sources {
        match row.disposition {
            SourceDisposition::ReadFailed => coverage.unreadable_bodies.push(row.record_index),
            SourceDisposition::ProjectionFailed => {
                coverage.incomplete_fields.push(row.record_index);
            }
            _ => {}
        }
    }
    for inspection in ledger.field_inspections {
        if !inspection.complete {
            coverage.incomplete_fields.push(inspection.record_index);
        }
    }
    for diagnostic in ledger.field_diagnostics {
        if diagnostic.severity != ExportDiagnosticSeverity::Information {
            for example in diagnostic.examples {
                let key = example
                    .subject()
                    .key()
                    .ok_or(ExportAccountingError::UnboundHealthSubject)?;
                coverage.incomplete_fields.push(key.index());
            }
        }
    }
    Ok(coverage)
}

fn reached_ammo<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    routes: &[WeaponProjectileAmmoSelection],
    cancel: Option<&CancelToken>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Vec<PreparedAmmoDefinition<'source>>, CatalogError> {
    let indices: BTreeSet<_> = routes
        .iter()
        .filter_map(WeaponProjectileAmmoSelection::selected_ammo_index)
        .filter_map(|index| index.value().copied())
        .collect();
    let projection = projection.for_kind(Kind::AmmoDefinitions);
    let mut ammo = Vec::new();
    for index in indices {
        if let Some(definition) = read_ammo(forge, &projection, index, cancel, coverage)? {
            ammo.push(definition);
        }
    }
    ammo_definition::order(&mut ammo);
    Ok(ammo)
}

fn read_ammo<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
    index: u32,
    cancel: Option<&CancelToken>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Option<PreparedAmmoDefinition<'source>>, CatalogError> {
    checkpoint(cancel)?;
    let record = forge
        .record(index)
        .ok_or(ExportAccountingError::MissingPhysicalReferenceTarget { index })?;
    let tree = match record.tree() {
        Ok(tree) => tree,
        Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
        Err(_) => {
            coverage.unreadable_bodies.push(index);
            return Ok(None);
        }
    };
    let input = projection.original();
    let admission = RecordObservation::capture(input, &record)?;
    let selection = AmmoSelection::for_reached_record(&record, None);
    let inspected = crate::export_field_walk::inspect_observed(
        Some(input),
        &tree,
        &admission,
        Some(Kind::AmmoDefinitions),
        selection.role(),
        cancel,
    )?;
    let (definition, notes) =
        ammo_definition::read(projection, &tree, &admission, cancel, selection)?;
    if inspected.field_inspections.iter().any(|row| !row.complete)
        || notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
    {
        coverage.incomplete_fields.push(index);
    }
    Ok(Some(definition))
}
