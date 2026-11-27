// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use p4kx_extraction::{CancelToken, DataForgeError, Filter, Record, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::ammo_definition::{self, AmmoSelection, PreparedAmmoDefinition};
use crate::catalog_v3::catalog_item_components::resolution::MagazineDependencies;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_evidence_lock;
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::scan_health::ScanError;
use crate::{CatalogError, ExportAccountingError, Kind};

/// Retains one admitted Ammo read per demanded physical Record during a catalog scan.
pub(super) struct AmmoReads<'options, 'input, 'source> {
    projection: &'input CatalogProjection<'input, 'source>,
    filter: Option<&'options Filter>,
    cancel: Option<&'options CancelToken>,
    slots: Mutex<HashMap<u32, Arc<AmmoSlot<'source>>>>,
    pub(super) magazines: MagazineDependencies<'options, 'source>,
    #[cfg(test)]
    materialized_reads: AtomicUsize,
}

#[derive(Default)]
struct AmmoSlot<'source> {
    state: Mutex<SlotState<'source>>,
}

#[derive(Default)]
struct SlotState<'source> {
    read: Option<AmmoRead<'source>>,
    dependency_observed: bool,
}

enum AmmoRead<'source> {
    Readable(Box<ReadableAmmo<'source>>),
    Unreadable(DataForgeError),
}

struct ReadableAmmo<'source> {
    inspection: FieldInspection,
    definition: Option<CanonicalAmmo<'source>>,
}

struct CanonicalAmmo<'source> {
    definition: PreparedAmmoDefinition<'source>,
    projection: ExportLedger,
}

impl<'options, 'input, 'source> AmmoReads<'options, 'input, 'source> {
    pub(super) fn new(
        projection: &'input CatalogProjection<'input, 'source>,
        filter: Option<&'options Filter>,
        cancel: Option<&'options CancelToken>,
    ) -> Self {
        Self {
            projection,
            filter,
            cancel,
            slots: Mutex::new(HashMap::new()),
            magazines: MagazineDependencies::new(filter),
            #[cfg(test)]
            materialized_reads: AtomicUsize::new(0),
        }
    }

    pub(super) fn selected(
        &self,
        record: &Record<'source>,
        ledger: &mut ExportLedger,
    ) -> Result<PreparedAmmoDefinition<'source>, ScanError> {
        self.with(record, |state, _| {
            let read = state
                .read
                .as_mut()
                .ok_or_else(|| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
            let ReadableAmmo {
                definition,
                inspection,
                ..
            } = match read {
                AmmoRead::Readable(read) => read.as_mut(),
                AmmoRead::Unreadable(error) => return Err(error.clone().into()),
            };
            let selected = definition
                .take()
                .filter(|row| row.definition.selection() == AmmoSelection::Selected)
                .ok_or_else(|| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
            retain_inspection(
                ledger,
                inspection,
                Kind::AmmoDefinitions,
                ExportSourceRole::Selected,
            );
            ledger.merge(selected.projection);
            Ok(selected.definition)
        })
    }

    pub(super) fn retain_dependency(
        &self,
        record: &Record<'source>,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        self.with(record, |state, admission| {
            let read = state
                .read
                .as_ref()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            if !state.dependency_observed {
                record_dependency(ledger, admission, read)?;
                state.dependency_observed = true;
            }
            Ok(())
        })
    }

    /// Releases canonical rows for required ammunition and magazine dependencies.
    pub(super) fn dependencies(
        &self,
        mut collect: impl FnMut(Picked<'source>) -> Result<(), CatalogError>,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        for row in self.dependency_rows(ledger)? {
            collect(Picked::AmmoDefinition(Box::new(row)))?;
        }
        for row in self.magazines.take(ledger)? {
            collect(Picked::AmmoMagazineDependency(Box::new(row)))?;
        }
        Ok(())
    }

    fn dependency_rows(
        &self,
        ledger: &mut ExportLedger,
    ) -> Result<Vec<PreparedAmmoDefinition<'source>>, CatalogError> {
        let slots = export_evidence_lock::lock(&self.slots)?;
        let mut rows = Vec::new();
        for slot in slots.values() {
            let mut state = export_evidence_lock::lock(&slot.state)?;
            if !state.dependency_observed {
                continue;
            }
            let Some(AmmoRead::Readable(read)) = &mut state.read else {
                continue;
            };
            if read
                .definition
                .as_ref()
                .is_some_and(|row| row.definition.selection() == AmmoSelection::Dependency)
                && let Some(retained) = read.definition.take()
            {
                retain_inspection(
                    ledger,
                    &read.inspection,
                    Kind::AmmoDefinitions,
                    ExportSourceRole::Dependency,
                );
                ledger.merge(retained.projection);
                rows.push(retained.definition);
            }
            drop(state);
        }
        drop(slots);
        Ok(rows)
    }

    fn with<T, E>(
        &self,
        record: &Record<'source>,
        use_read: impl FnOnce(&mut SlotState<'source>, &RecordObservation<'_, 'source>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CatalogError>,
    {
        let admission = RecordObservation::capture(self.projection.original(), record)?;
        let slot = {
            let mut slots = export_evidence_lock::lock(&self.slots).map_err(CatalogError::from)?;
            Arc::clone(slots.entry(record.index()).or_default())
        };
        let mut state = export_evidence_lock::lock(&slot.state).map_err(CatalogError::from)?;
        if state.read.is_none() {
            #[cfg(test)]
            self.materialized_reads.fetch_add(1, Ordering::Relaxed);
            let read = match record.tree() {
                Ok(tree) => AmmoRead::Readable(Box::new(self.prepare(&tree, &admission)?)),
                Err(error) => AmmoRead::Unreadable(error),
            };
            state.read = Some(read);
        }
        let result = use_read(&mut state, &admission);
        drop(state);
        result
    }

    fn prepare(
        &self,
        tree: &RecordTree<'source>,
        admission: &RecordObservation<'_, 'source>,
    ) -> Result<ReadableAmmo<'source>, CatalogError> {
        let mut inspected = crate::export_field_walk::inspect_observed(
            Some(self.projection.original()),
            tree,
            admission,
            None,
            ExportSourceRole::Dependency,
            self.cancel,
        )?;
        let inspection = inspected
            .field_inspections
            .pop()
            .ok_or(ExportAccountingError::ConflictingFieldInspection)?;
        let selection = AmmoSelection::for_reached_record(admission.record(), self.filter);
        let definition = self.prepare_definition(tree, admission, selection)?;
        Ok(ReadableAmmo {
            inspection,
            definition: Some(definition),
        })
    }

    fn prepare_definition(
        &self,
        tree: &RecordTree<'source>,
        admission: &RecordObservation<'_, 'source>,
        selection: AmmoSelection,
    ) -> Result<CanonicalAmmo<'source>, CatalogError> {
        let (definition, notes) = ammo_definition::read(
            &self.projection.for_kind(Kind::AmmoDefinitions),
            tree,
            admission,
            self.cancel,
            selection,
        )?;
        let mut projection = ExportLedger::default();
        crate::source_projection_notes::record(
            Kind::AmmoDefinitions,
            selection.role(),
            admission,
            tree,
            notes,
            &mut projection,
        )?;
        if selection == AmmoSelection::Dependency {
            projection.observe(admission.source(
                Some(Kind::AmmoDefinitions),
                ExportSourceRole::Dependency,
                SourceDisposition::Projected,
            )?);
        }
        Ok(CanonicalAmmo {
            definition,
            projection,
        })
    }
}

fn record_dependency(
    ledger: &mut ExportLedger,
    admission: &RecordObservation<'_, '_>,
    read: &AmmoRead<'_>,
) -> Result<(), CatalogError> {
    let kind = Kind::Items;
    let role = ExportSourceRole::Dependency;
    match read {
        AmmoRead::Readable(read) => {
            retain_inspection(ledger, &read.inspection, kind, role);
            ledger.observe(admission.source(Some(kind), role, SourceDisposition::Projected)?);
        }
        AmmoRead::Unreadable(error) => ledger.observe(
            admission
                .source(Some(kind), role, SourceDisposition::ReadFailed)?
                .diagnosed(crate::export_source_failure::diagnosis(error)),
        ),
    }
    Ok(())
}

fn retain_inspection(
    ledger: &mut ExportLedger,
    inspection: &FieldInspection,
    kind: Kind,
    role: ExportSourceRole,
) {
    let mut inspection = inspection.clone();
    inspection.contextualize(Some(kind), role);
    ledger.field_inspections.push(inspection);
}

#[cfg(test)]
#[path = "scan_ammo_tests.rs"]
mod tests;
