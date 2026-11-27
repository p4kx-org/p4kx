// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    ArchiveFiles, LoadoutError, VehicleImplementation, VehicleStructureEvidence,
    read_observed_vehicle_tree,
};
use p4kx_extraction::{Archive, Record, RecordTree, SourceUseRole};

use super::super::vehicle_capture;
use super::{Capture, Contributions, VehicleFiles};
use crate::catalog_read::original::EntryCapture;
use crate::digest::Digest;
use crate::variant_evidence::VariantEvidence;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Decision {
    Digest(Digest),
    NoConfiguredVehicle,
    MissingImplementation,
    MissingExplicitPatch,
    Unavailable,
}

pub(crate) struct VehicleStructure {
    pub(crate) decision: Decision,
    pub(crate) evidence: VariantEvidence,
}

#[derive(Clone, Copy)]
pub(crate) struct Operands<'input> {
    pub(crate) record_index: u32,
    pub(crate) vehicle: &'input p4kx_extraction::SourcedValue<bool>,
    pub(crate) modification: &'input p4kx_extraction::SourcedValue<bool>,
}

/// Binds one original observed Vehicle read and its reported comparison outcome.
pub(crate) fn read<'source>(
    capture: &Arc<Capture<'source>>,
    files: &ArchiveFiles<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    operands: Operands<'_>,
) -> Result<VehicleStructure, CatalogError> {
    if operands.record_index != record.index()
        || capture.kind != crate::Kind::Loadouts
        || !files.verifies_entries()
    {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let input = capture
        .input
        .ok_or(ExportAccountingError::UnboundHealthSubject)?;
    capture.check_record(record)?;
    capture.begin_loadout_vehicles()?;
    let outcome = read_observed_vehicle_tree(input.forge(), record, tree, files, capture.clone());
    capture.finish()?;
    bind(capture, files.archive(), record.index(), operands, outcome)
}

fn bind(
    capture: &Capture<'_>,
    archive: &Archive,
    record_index: u32,
    operands: Operands<'_>,
    outcome: Result<Option<Arc<VehicleImplementation>>, LoadoutError>,
) -> Result<VehicleStructure, CatalogError> {
    let input = capture
        .input
        .ok_or(ExportAccountingError::UnboundHealthSubject)?;
    let state = capture.lock()?;
    let sourced_definition = vehicle_capture::known(operands.vehicle);
    let configured = operands.vehicle.value() == Some(&true);
    let sourced_modification = vehicle_capture::known(operands.modification);
    let entries = EntryCapture::for_input(input);
    entries.check_archive(archive.input_origin())?;
    let mut proof = Contributions::default();
    let decision = match outcome {
        Ok(None) if sourced_definition && !configured => Decision::NoConfiguredVehicle,
        Ok(None) => Decision::Unavailable,
        Ok(Some(vehicle)) => {
            let files = VehicleFiles {
                state: &state,
                entries: Some(entries),
                call: state.port_vehicles.call(record_index),
            };
            let directory_complete = files.observe_file_choices(Some(archive), &mut proof)?;
            let operands_complete = sourced_definition && configured && sourced_modification;
            let decision = if operands_complete
                && files.verified_explicit_patch_absence(&vehicle, directory_complete)
            {
                Decision::MissingExplicitPatch
            } else {
                proved_digest(&vehicle, operands_complete)
            };
            files.bind_structure(&vehicle, &decision, &mut proof)?;
            decision
        }
        Err(error) => {
            let (call, missing) = match error {
                LoadoutError::MissingVehicleImplementation { path } => (
                    state.missing_port_vehicle(input, record_index, &path)?,
                    true,
                ),
                LoadoutError::ObservationStopped => return Err(CatalogError::Cancelled),
                _ => (state.failed_port_vehicle(input, record_index)?, false),
            };
            let files = VehicleFiles {
                state: &state,
                entries: Some(entries),
                call: call.as_ref(),
            };
            files.observe_file_choices(Some(archive), &mut proof)?;
            if missing && call.is_some() && configured && sourced_definition {
                Decision::MissingImplementation
            } else {
                Decision::Unavailable
            }
        }
    };
    drop(state);
    finish(&capture.name_sources, operands, decision, proof)
}

fn proved_digest(vehicle: &VehicleImplementation, complete: bool) -> Decision {
    if complete && let Ok(digest) = vehicle.complete_structure_digest() {
        return Decision::Digest(Digest::from_bytes(digest));
    }
    Decision::Unavailable
}

fn finish(
    sources: &p4kx_extraction::SourceUses,
    operands: Operands<'_>,
    decision: Decision,
    proof: Contributions,
) -> Result<VehicleStructure, CatalogError> {
    let (uses, complete) = proof.into_parts();
    let mut evidence = VariantEvidence::from_uses(
        sources,
        &uses,
        complete && decision != Decision::Unavailable,
    );
    let consulted_modification = matches!(
        decision,
        Decision::Digest(_) | Decision::MissingExplicitPatch | Decision::Unavailable
    )
    .then_some(operands.modification.origin());
    evidence.origin = sources.origin_with_origins(
        [&evidence.origin, operands.vehicle.origin()]
            .into_iter()
            .chain(consulted_modification),
        true,
    )?;
    let decision = if evidence.origin.source_set_complete() {
        decision
    } else {
        Decision::Unavailable
    };
    Ok(VehicleStructure { decision, evidence })
}

impl VehicleFiles<'_> {
    fn bind_structure(
        &self,
        vehicle: &VehicleImplementation,
        decision: &Decision,
        proof: &mut Contributions,
    ) -> Result<(), CatalogError> {
        let Some(evidence) = vehicle.structure_evidence() else {
            proof.incomplete();
            return Ok(());
        };
        let complete = match decision {
            Decision::MissingExplicitPatch => evidence.missing_explicit_patch_is_only_gap(),
            _ => evidence.is_complete(),
        };
        if !complete {
            proof.incomplete();
        }
        self.bind_structure_documents(&evidence, proof)?;
        for site in evidence.selectors() {
            self.retain(site, SourceUseRole::Selector, proof)?;
        }
        Ok(())
    }

    fn bind_structure_documents(
        &self,
        evidence: &VehicleStructureEvidence<'_>,
        proof: &mut Contributions,
    ) -> Result<(), CatalogError> {
        let (Some(entries), Some(call)) = (&self.entries, self.call) else {
            proof.incomplete();
            return Ok(());
        };
        let mut applied = 0;
        for selected in &call.files {
            let committed = evidence
                .applied_documents()
                .any(|file| selected.matches(file, &call.caller));
            let role = if committed {
                SourceUseRole::Conversion
            } else {
                SourceUseRole::Attempted
            };
            if let Some(document) = selected.projected_payload(entries, role)? {
                proof.add(document);
                applied += usize::from(committed);
            }
        }
        if applied != evidence.applied_documents().count() {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "vehicle_structure_binding_tests.rs"]
mod tests;
