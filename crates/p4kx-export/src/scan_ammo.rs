// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use p4kx_extraction::{CancelToken, DataForgeError, Filter, Record, RecordTree};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_v3::ammo_definition::{self, PreparedAmmoDefinition};
use crate::contract_stats::AmmoStats;
use crate::entity_selection::{self, Selected};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_evidence_lock;
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::scan_health::ScanError;
use crate::{CatalogError, ExportAccountingError, Kind, item_stats};

/// Retains one admitted Ammo read per demanded physical Record during a catalog scan.
pub(super) struct AmmoReads<'options, 'input, 'source> {
    input: &'input OriginalCatalogInput<'input, 'source>,
    filter: Option<&'options Filter>,
    cancel: Option<&'options CancelToken>,
    slots: Mutex<HashMap<u32, Arc<AmmoSlot<'source>>>>,
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
    compatibility: AmmoStats,
    inspection: FieldInspection,
    selected: Option<SelectedAmmo<'source>>,
}

struct SelectedAmmo<'source> {
    definition: PreparedAmmoDefinition<'source>,
    projection: ExportLedger,
}

impl<'options, 'input, 'source> AmmoReads<'options, 'input, 'source> {
    pub(super) fn new(
        input: &'input OriginalCatalogInput<'input, 'source>,
        filter: Option<&'options Filter>,
        cancel: Option<&'options CancelToken>,
    ) -> Self {
        Self {
            input,
            filter,
            cancel,
            slots: Mutex::new(HashMap::new()),
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
                selected,
                inspection,
                ..
            } = match read {
                AmmoRead::Readable(read) => read.as_mut(),
                AmmoRead::Unreadable(error) => return Err(error.clone().into()),
            };
            let selected = selected
                .take()
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

    pub(super) fn dependency(
        &self,
        record: &Record<'source>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<AmmoStats>, CatalogError> {
        self.with(record, |state, admission| {
            let read = state
                .read
                .as_ref()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            if !state.dependency_observed {
                retain_dependency(ledger, admission, read)?;
                state.dependency_observed = true;
            }
            Ok(match read {
                AmmoRead::Readable(read) => Some(read.compatibility.clone()),
                AmmoRead::Unreadable(_) => None,
            })
        })
    }

    fn with<T, E>(
        &self,
        record: &Record<'source>,
        use_read: impl FnOnce(&mut SlotState<'source>, &RecordObservation<'_, 'source>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CatalogError>,
    {
        let admission = RecordObservation::capture(self.input, record)?;
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
        let compatibility = item_stats::ammo(tree);
        let mut inspected = crate::export_field_walk::inspect_observed(
            Some(self.input),
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
        let selected = matches!(
            entity_selection::select(admission.record(), self.filter),
            Ok(Some(Selected::Entity(Kind::AmmoDefinitions)))
        )
        .then(|| self.prepare_selected(tree, admission))
        .transpose()?;
        Ok(ReadableAmmo {
            compatibility,
            inspection,
            selected,
        })
    }

    fn prepare_selected(
        &self,
        tree: &RecordTree<'source>,
        admission: &RecordObservation<'_, 'source>,
    ) -> Result<SelectedAmmo<'source>, CatalogError> {
        let (definition, notes) = ammo_definition::read(self.input, tree, admission, self.cancel)?;
        let mut projection = ExportLedger::default();
        crate::source_projection_notes::record(
            Kind::AmmoDefinitions,
            ExportSourceRole::Selected,
            admission,
            tree,
            notes,
            &mut projection,
        )?;
        Ok(SelectedAmmo {
            definition,
            projection,
        })
    }
}

fn retain_dependency(
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
