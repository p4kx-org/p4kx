// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    ArchiveFiles, LoadoutError, VehicleImplementation, read_observed_vehicle_tree,
};
use p4kx_extraction::{CancelToken, RecordTree, SourceEvidenceError};

use super::record_notes;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_v3::authored_fields::ship_source_read::{self, ShipSourceRead};
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::{Capture, ShipVehicleValues};
use crate::record_observation::RecordObservation;
use crate::ship_base::VehicleMeasurements;
use crate::{CatalogError, CatalogReadFailure, Kind};

pub(crate) struct ProjectedShip<'read, 'source> {
    pub(crate) source: ShipSourceRead<'read, 'source>,
    pub(crate) vehicle: ShipVehicleValues,
}

/// Projects the selected original Ship without resolving any Loadout or Cargo dependency.
pub(crate) fn read<'read, 'source>(
    files: &ArchiveFiles<'source>,
    projection: &'read CatalogProjection<'_, 'source>,
    input: &'source OriginalCatalogInput<'source, 'source>,
    admission: &'read RecordObservation<'_, 'source>,
    tree: &'read RecordTree<'source>,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Result<ProjectedShip<'read, 'source>, CatalogReadFailure<'source>>, CatalogError> {
    checkpoint(cancel)?;
    if !std::ptr::eq(projection.original(), input) {
        return Err(SourceEvidenceError::ForeignEvidence.into());
    }
    EntryCapture::for_input(input).check_archive(files.archive().input_origin())?;
    let mut source = ship_source_read::read(tree, projection, admission, cancel)?;
    let record = admission.record();
    let capture = Arc::new(Capture::for_input(input, ledger, cancel, Kind::Ships));
    capture.begin_vehicle(record, &source.aggregate)?;
    let vehicle = read_observed_vehicle_tree(input.forge(), record, tree, files, capture.clone());
    let vehicle = match implementation(vehicle) {
        Ok(vehicle) => vehicle,
        Err(LoadoutError::ObservationStopped) => {
            capture.finish()?;
            return Err(CatalogError::Cancelled);
        }
        Err(error) => {
            ledger.merge(capture.finish()?);
            return Ok(Err(CatalogReadFailure::of(record, &error)));
        }
    };
    let totals = vehicle
        .as_deref()
        .map(|vehicle| capture.vehicle_totals(record.index(), vehicle));
    let mut physics_used = false;
    let measured = VehicleMeasurements::read(tree, vehicle.as_deref(), totals.as_ref(), || {
        physics_used = true;
        source.aggregate.physics.value().copied()
    });
    let vehicle = capture.finish_direct_vehicle(files.archive(), record, &measured, physics_used);
    ledger.merge(capture.finish()?);
    let vehicle = vehicle?;
    record_notes(&mut source, admission, tree, vehicle.physics_used, ledger)?;
    checkpoint(cancel)?;
    Ok(Ok(ProjectedShip { source, vehicle }))
}

fn implementation(
    result: Result<Option<Arc<VehicleImplementation>>, LoadoutError>,
) -> Result<Option<Arc<VehicleImplementation>>, LoadoutError> {
    match result {
        Err(
            LoadoutError::MissingVehicleImplementation { .. }
            | LoadoutError::VehicleImplementation { .. },
        ) => Ok(None),
        other => other,
    }
}
