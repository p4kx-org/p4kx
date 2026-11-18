// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "loadout_ledger/alignment.rs"]
mod alignment;
#[path = "loadout_ledger/file_dependencies.rs"]
mod file_dependencies;
#[path = "loadout_ledger/file_proofs.rs"]
mod file_proofs;
#[path = "loadout_ledger/files.rs"]
mod files;
#[cfg(test)]
#[path = "loadout_ledger/name_tests.rs"]
mod name_tests;
#[path = "loadout_ledger/observer.rs"]
mod observer;
#[path = "loadout_ledger/operation.rs"]
mod operation;
#[path = "loadout_ledger/path_capture.rs"]
mod path_capture;
#[path = "loadout_ledger/path_observer.rs"]
mod path_observer;
#[path = "loadout_ledger/path_proofs.rs"]
mod path_proofs;
#[path = "loadout_ledger/port_attributes.rs"]
mod port_attributes;
#[path = "loadout_ledger/root_index.rs"]
mod root_index;
#[path = "loadout_ledger/typed.rs"]
mod typed;
#[path = "loadout_ledger/vehicle_capture.rs"]
mod vehicle_capture;
#[path = "loadout_ledger/vehicle_fields.rs"]
mod vehicle_fields;
#[path = "loadout_ledger/vehicle_result.rs"]
mod vehicle_result;
#[path = "loadout_ledger/vehicle_stage.rs"]
mod vehicle_stage;

pub(crate) use vehicle_result::ShipAggregateValues;

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};
use std::ops::ControlFlow;
use std::sync::{Mutex, MutexGuard};

use p4kx_extraction::{CancelToken, SourceUses};

use self::file_proofs::FileProofs;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::FileGraph;
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{ExportAccountingError, ExportLedger};
use crate::record_observation::RecordObservation;

struct State {
    ledger: ExportLedger,
    inspections: BTreeMap<u32, FieldInspection>,
    files: FileProofs,
    paths: path_proofs::PathProofs,
    port_attributes: port_attributes::RecordPortAttributes,
    references: reference_proofs::ReferenceProofs,
    classes: class_proofs::ClassProofs,
    graph: FileGraph,
    vehicles: vehicle_stage::VehicleCapture,
    seen: HashSet<u32>,
    proofs: HashSet<u32>,
    error: Option<CatalogError>,
}

impl State {
    fn new(source: &ExportLedger) -> Self {
        let mut state = Self {
            ledger: ExportLedger::default(),
            inspections: BTreeMap::new(),
            files: FileProofs::default(),
            paths: path_proofs::PathProofs::default(),
            port_attributes: port_attributes::RecordPortAttributes::default(),
            references: reference_proofs::ReferenceProofs::default(),
            classes: class_proofs::ClassProofs::default(),
            graph: FileGraph::default(),
            vehicles: vehicle_stage::VehicleCapture::default(),
            seen: HashSet::new(),
            proofs: HashSet::new(),
            error: None,
        };
        state.seed(source);
        state
    }

    fn merge_canonical(&mut self, mut ledger: ExportLedger) -> Result<(), ExportAccountingError> {
        for inspection in std::mem::take(&mut ledger.field_inspections) {
            match self.inspections.entry(inspection.record_index) {
                Entry::Vacant(slot) => {
                    slot.insert(inspection);
                }
                Entry::Occupied(mut slot) => slot.get_mut().merge(inspection)?,
            }
        }
        self.ledger.merge(ledger);
        Ok(())
    }

    fn seed(&mut self, source: &ExportLedger) {
        for proof in &source.field_inspections {
            self.seen.insert(proof.record_index);
            self.proofs.insert(proof.record_index);
        }
    }
}

/// Retains resolver proofs for cache lifetime and drains each caller's ledger delta.
pub(crate) struct Capture<'f> {
    input: Option<&'f OriginalCatalogInput<'f, 'f>>,
    name_sources: SourceUses,
    kind: crate::Kind,
    state: Mutex<State>,
    cancel: Option<CancelToken>,
}

impl<'f> Capture<'f> {
    #[cfg(test)]
    pub(crate) fn new(source: &ExportLedger, cancel: Option<&CancelToken>) -> Self {
        Self::create(source, cancel, crate::Kind::Loadouts, SourceUses::default())
    }

    pub(crate) fn for_input(
        input: &'f OriginalCatalogInput<'f, 'f>,
        source: &ExportLedger,
        cancel: Option<&CancelToken>,
        kind: crate::Kind,
    ) -> Self {
        let mut capture = Self::create(source, cancel, kind, input.origins().uses.clone());
        capture.input = Some(input);
        capture
    }

    pub(crate) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), CatalogError> {
        if self
            .input
            .is_none_or(|original| !std::ptr::eq(original, input))
        {
            return Err(p4kx_extraction::SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(())
    }

    fn admission<'read, 'source>(
        &self,
        record: &'read p4kx_extraction::Record<'source>,
    ) -> Result<RecordObservation<'read, 'source>, CatalogError> {
        self.input.map_or_else(
            || Ok(RecordObservation::accounting(record)),
            |input| RecordObservation::capture(input, record),
        )
    }

    fn check_record(&self, record: &p4kx_extraction::Record<'_>) -> Result<(), CatalogError> {
        if let Some(input) = self.input {
            input.record_context(record)?;
        }
        Ok(())
    }

    fn create(
        source: &ExportLedger,
        cancel: Option<&CancelToken>,
        kind: crate::Kind,
        name_sources: SourceUses,
    ) -> Self {
        Self {
            input: None,
            name_sources,
            kind,
            state: Mutex::new(State::new(source)),
            cancel: cancel.cloned(),
        }
    }

    pub(crate) fn seed_proofs(&self, source: &ExportLedger) -> Result<(), CatalogError> {
        self.lock()?.seed(source);
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, ExportAccountingError> {
        crate::export_evidence_lock::lock(&self.state)
    }

    pub(super) fn record_error(&self, error: CatalogError) {
        if let Ok(mut state) = self.lock() {
            state.error.get_or_insert(error);
        }
    }

    fn fail(&self, error: CatalogError) -> ControlFlow<()> {
        self.record_error(error);
        ControlFlow::Break(())
    }

    fn stopped(&self) -> bool {
        self.cancel.as_ref().is_some_and(CancelToken::is_cancelled)
            || self.lock().map_or(true, |state| state.error.is_some())
    }

    pub(crate) fn missing_root(&self) {
        self.record_error(ExportAccountingError::MissingFieldInspection.into());
    }

    /// Propagates observer, poisoning or cancellation failures before publication.
    pub(crate) fn finish(&self) -> Result<ExportLedger, CatalogError> {
        let mut state = self.lock()?;
        if let Some(error) = state.error.take() {
            return Err(error);
        }
        if self.cancel.as_ref().is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let mut ledger = std::mem::take(&mut state.ledger);
        let inspections = std::mem::take(&mut state.inspections);
        drop(state);
        ledger.field_inspections.extend(inspections.into_values());
        Ok(ledger)
    }
}

#[cfg(test)]
#[path = "loadout_ledger/inspection_tests.rs"]
mod inspection_tests;
#[cfg(test)]
#[path = "loadout_ledger/tests.rs"]
mod tests;

#[path = "loadout_ledger/request_examples.rs"]
mod request_examples;

#[path = "loadout_ledger/reference_capture.rs"]
mod reference_capture;
#[path = "loadout_ledger/reference_proofs.rs"]
mod reference_proofs;

#[path = "loadout_ledger/class_capture.rs"]
mod class_capture;
#[path = "loadout_ledger/class_proofs.rs"]
mod class_proofs;
