// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{
    LoadoutFileObservation, LoadoutRequest, VehicleAggregateIssue, VehicleImplementation,
    VehicleXmlSite,
};
use p4kx_extraction::{Record, SourceUseRole, SourceValueState};

use super::path_proofs::PathProofs;
use super::vehicle_stage::{Contributions, MetricContributions, PendingVehicle, SelectedVehicle};
use super::{Capture, State};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{EntryCapture, RecordSubject, RecordSubjectKey, VehicleCaller};
use crate::catalog_v3::authored_fields::ship_source_read::ShipAggregateOperands;
use crate::record_header_field::RecordHeaderInput;
use crate::ship_base::VehicleKindPath;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn caller(
    paths: &PathProofs,
    input: &OriginalCatalogInput<'_, '_>,
    subject: &RecordSubject,
    request: &LoadoutRequest,
) -> Result<VehicleCaller, CatalogError> {
    let LoadoutRequest::Vehicle {
        definition,
        modification,
        class_name: Some(class_name),
    } = request
    else {
        return Err(ExportAccountingError::UnboundHealthSubject.into());
    };
    VehicleCaller::read(
        input,
        subject,
        paths.get(definition)?,
        modification
            .selection()
            .map(|_| paths.get(modification))
            .transpose()?,
        paths.get(class_name)?,
    )
}

impl State {
    pub(super) fn stage_vehicle_file(
        &mut self,
        input: Option<&OriginalCatalogInput<'_, '_>>,
        original: &LoadoutFileObservation,
        request: &LoadoutRequest,
    ) -> Result<(), CatalogError> {
        let Some(pending) = &mut self.vehicles.pending else {
            return Ok(());
        };
        let LoadoutRequest::Vehicle { class_name, .. } = request else {
            return Ok(());
        };
        let input = input.ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let class_name = class_name
            .as_ref()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let index = self.paths.get(class_name)?.record_owner()?;
        if Some(index) != pending.subject.key().map(RecordSubjectKey::index) {
            return Ok(());
        }
        let caller = caller(&self.paths, input, &pending.subject, request)?;
        if !pending
            .files
            .iter()
            .any(|file| file.matches(original, &caller))
        {
            pending
                .files
                .push(self.graph.vehicle_selection(original, caller)?);
        }
        Ok(())
    }
}

impl Capture<'_> {
    pub(crate) fn begin_vehicle(
        &self,
        record: &Record<'_>,
        operands: &ShipAggregateOperands,
    ) -> Result<(), CatalogError> {
        let Some(input) = self.input else {
            return Err(ExportAccountingError::UnboundHealthSubject.into());
        };
        if self.kind != crate::Kind::Ships {
            return Err(ExportAccountingError::UnboundHealthSubject.into());
        }
        self.check_record(record)?;
        if operands.record_index != record.index() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let subject = self.admission(record)?.subject().clone();
        let record_path = record
            .record_path()
            .map_err(|_| ExportAccountingError::MissingFieldInspection)?;
        let mut path_uses = Vec::new();
        input.origins().record(record.index()).append_header_input(
            RecordHeaderInput::NormalizedPath,
            SourceUseRole::Selector,
            &mut path_uses,
        )?;
        let kind_path_origin = input.origins().uses.origin(&path_uses, true);
        let mut state = self.lock()?;
        if state.vehicles.pending.is_some() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        state.vehicles.pending = Some(PendingVehicle {
            subject,
            operands: operands.clone(),
            kind_path: VehicleKindPath::of(record_path),
            kind_path_origin,
            files: Vec::new(),
            selected: None,
            mass: MetricContributions::default(),
            hull: MetricContributions::default(),
            totals_seen: false,
        });
        drop(state);
        Ok(())
    }

    pub(super) fn capture_vehicle(
        &self,
        record_index: u32,
        implementation: &VehicleImplementation,
        request: &LoadoutRequest,
    ) -> ControlFlow<()> {
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        let result = (|| {
            let input = self
                .input
                .ok_or(ExportAccountingError::UnboundHealthSubject)?;
            let mut state = self.lock()?;
            let Some(pending) = &state.vehicles.pending else {
                return Ok(());
            };
            if pending.subject.key().map(RecordSubjectKey::index) != Some(record_index) {
                return Ok(());
            }
            if pending.selected.is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            let caller = caller(&state.paths, input, &pending.subject, request)?;
            let original = implementation
                .aggregate_evidence()
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            let entries = EntryCapture::for_input(input);
            let mut membership =
                bind_sites(&state, pending, &caller, &entries, original.selectors())?;
            let rolling = rolling_part_proof(&state, pending, &caller, &entries, implementation)?;
            let unavailable_inputs = original.issues().iter().copied().any(unreadable_stage);
            if !original.has_complete_coordinates() || unavailable_inputs {
                membership.incomplete();
            }
            state
                .vehicles
                .pending
                .as_mut()
                .ok_or(ExportAccountingError::UnobservedFailure)?
                .selected = Some(SelectedVehicle {
                caller,
                membership,
                rolling,
                unavailable_inputs,
            });
            drop(state);
            Ok::<_, CatalogError>(())
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }
}

fn rolling_part_proof(
    state: &State,
    pending: &PendingVehicle,
    caller: &VehicleCaller,
    entries: &EntryCapture<'_>,
    implementation: &VehicleImplementation,
) -> Result<Option<Contributions>, CatalogError> {
    if pending.kind_path != VehicleKindPath::Space {
        return Ok(None);
    }
    let mut proof = bind_sites(
        state,
        pending,
        caller,
        entries,
        implementation.rolling_part_selectors(),
    )?;
    if !implementation.rolling_part_selectors_complete() {
        proof.incomplete();
    }
    Ok(Some(proof))
}

fn bind_sites<'site>(
    state: &State,
    pending: &PendingVehicle,
    caller: &VehicleCaller,
    entries: &EntryCapture<'_>,
    sites: impl Iterator<Item = &'site VehicleXmlSite>,
) -> Result<Contributions, CatalogError> {
    let mut proof = Contributions::default();
    for site in sites {
        if !state.files.contains(site.file()) {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        let selected = pending
            .files
            .iter()
            .find(|file| file.matches(site.file(), caller))
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        proof.add(selected.site(entries, site, SourceUseRole::Selector)?);
    }
    Ok(proof)
}

pub(super) fn known<T>(value: &p4kx_extraction::SourcedValue<T>) -> bool {
    matches!(
        value.state(),
        SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
    ) && value.origin().source_set_complete()
}

const fn unreadable_stage(issue: VehicleAggregateIssue) -> bool {
    !matches!(
        issue,
        VehicleAggregateIssue::ModificationUnavailable
            | VehicleAggregateIssue::ExplicitPatchMissing
            | VehicleAggregateIssue::PatchPathRejected
            | VehicleAggregateIssue::OriginalEvidenceUnavailable
    )
}
