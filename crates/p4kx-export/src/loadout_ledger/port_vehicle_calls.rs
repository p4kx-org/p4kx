// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::ControlFlow;

use p4kx_assembly::{LoadoutFileObservation, LoadoutRequest, LoadoutSelection};

use super::path_proofs::PathProofs;
use super::vehicle_capture;
use super::{Capture, State};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{VehicleCaller, VehicleFileSelection};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

#[derive(Default)]
pub(super) struct PortVehicles {
    pending: HashMap<LoadoutSelection, Vec<LoadoutFileObservation>>,
    calls: HashMap<u32, PortVehicleCall>,
}

pub(super) struct PortVehicleCall {
    pub(super) caller: VehicleCaller,
    pub(super) modification_observed: bool,
    pub(super) files: Vec<VehicleFileSelection>,
}

impl PortVehicles {
    pub(super) fn observe(&mut self, file: &LoadoutFileObservation, request: &LoadoutRequest) {
        let Some(selection) = selection(request) else {
            return;
        };
        let files = self.pending.entry(selection.clone()).or_default();
        if !files.iter().any(|seen| seen.is_same_operation(file)) {
            files.push(file.clone());
        }
    }

    pub(super) fn call(&self, index: u32) -> Option<&PortVehicleCall> {
        self.calls.get(&index)
    }
}

impl Capture<'_> {
    pub(crate) fn begin_loadout_vehicles(&self) -> Result<(), CatalogError> {
        self.lock()?.port_vehicles = PortVehicles::default();
        Ok(())
    }

    pub(super) fn bind_loadout_vehicle(
        &self,
        record_index: u32,
        request: &LoadoutRequest,
    ) -> ControlFlow<()> {
        let Some(input) = self.input else {
            return ControlFlow::Continue(());
        };
        let result = self
            .lock()
            .map_err(CatalogError::from)
            .and_then(|mut state| state.bind_port_vehicle(input, record_index, request));
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }
}

impl State {
    fn bind_port_vehicle(
        &mut self,
        input: &OriginalCatalogInput<'_, '_>,
        record_index: u32,
        request: &LoadoutRequest,
    ) -> Result<(), CatalogError> {
        let Some(selection) = selection(request) else {
            self.port_vehicles.calls.remove(&record_index);
            return Ok(());
        };
        let Some(observed) = self.port_vehicles.pending.remove(selection) else {
            self.port_vehicles.calls.remove(&record_index);
            return Ok(());
        };
        let caller = caller(input, &self.paths, record_index, request)?;
        let modification_observed = matches!(
            request,
            LoadoutRequest::Vehicle { modification, .. } if modification.selection().is_some()
        );
        let files = observed
            .iter()
            .map(|file| self.graph.vehicle_selection(file, caller.clone()))
            .collect::<Result<_, _>>()?;
        self.port_vehicles.calls.insert(
            record_index,
            PortVehicleCall {
                caller,
                modification_observed,
                files,
            },
        );
        Ok(())
    }
}

const fn selection(request: &LoadoutRequest) -> Option<&LoadoutSelection> {
    let LoadoutRequest::Vehicle {
        class_name: Some(class_name),
        ..
    } = request
    else {
        return None;
    };
    class_name.selection()
}

fn caller(
    input: &OriginalCatalogInput<'_, '_>,
    paths: &PathProofs,
    record_index: u32,
    request: &LoadoutRequest,
) -> Result<VehicleCaller, CatalogError> {
    let original = input
        .forge()
        .record(record_index)
        .ok_or(ExportAccountingError::UnboundHealthSubject)?;
    let admission = RecordObservation::capture(input, &original)?;
    vehicle_capture::caller(paths, input, admission.subject(), request)
}
