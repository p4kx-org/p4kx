// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, FieldOrigin, Record, SourceUses, SourceValue, SourceValueState, SourcedValue,
};

use super::Capture;
use super::vehicle_capture;
use super::vehicle_stage::{Contributions, PendingVehicle};
use crate::catalog_read::original::{EntryCapture, RecordSubjectKey, VehicleFileSelection};
use crate::ship_base::{
    Hull, Mass, ShipCargoObservation, ShipCargoSource, ShipRead, VehicleKindPath,
};
use crate::{CatalogError, ExportAccountingError, MassBasis, VehicleKind};

#[cfg(test)]
#[path = "vehicle_result_scope_tests.rs"]
mod scope_tests;

/// The direct vehicle measurements and the independently resolved Cargo.
pub(crate) struct ShipAggregateValues {
    pub(crate) vehicle: ShipVehicleValues,
    pub(crate) cargo: ShipCargoSource,
}

/// The direct vehicle values with their original implementation and Physics evidence.
pub(crate) struct ShipVehicleValues {
    pub(crate) vehicle_kind: SourcedValue<VehicleKind>,
    pub(crate) mass: SourcedValue<f64>,
    pub(crate) mass_basis: SourcedValue<MassBasis>,
    pub(crate) hull_health: SourcedValue<f64>,
    pub(crate) physics_used: bool,
}

struct MeasuredVehicle<'read> {
    kind_path: VehicleKindPath,
    kind: Option<VehicleKind>,
    mass: Option<&'read Mass>,
    hull: Option<&'read Hull>,
    physics_used: bool,
}

impl Capture<'_> {
    pub(crate) fn finish_vehicle(
        &self,
        archive: &Archive,
        record: &Record<'_>,
        read: Option<&ShipRead>,
        physics_used: bool,
        cargo: ShipCargoObservation<'_, '_>,
    ) -> Result<Option<ShipAggregateValues>, CatalogError> {
        let Some(pending) = self.take_vehicle(record, read.is_some())? else {
            return Ok(None);
        };
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let read = read.ok_or(ExportAccountingError::UnobservedFailure)?;
        let base = &read.base;
        let measurement = MeasuredVehicle {
            kind_path: VehicleKindPath::of(&base.record_path),
            kind: base.vehicle_kind,
            mass: base.mass.as_ref(),
            hull: base.hull.as_ref(),
            physics_used,
        };
        let cargo = cargo.finish(
            &base.cargo,
            &pending.cargo_membership(input, self.cancel.as_ref())?,
        )?;
        let vehicle = bind_values(
            &pending,
            &measurement,
            &EntryCapture::for_input(input),
            archive,
        )?;
        Ok(Some(ShipAggregateValues { vehicle, cargo }))
    }

    #[cfg(test)]
    pub(crate) fn finish_direct_vehicle(
        &self,
        archive: &Archive,
        record: &Record<'_>,
        measurements: &crate::ship_base::VehicleMeasurements,
        physics_used: bool,
    ) -> Result<ShipVehicleValues, CatalogError> {
        let pending = self
            .take_vehicle(record, true)?
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        bind_values(
            &pending,
            &MeasuredVehicle {
                kind_path: measurements.path_kind,
                kind: measurements.kind,
                mass: measurements.mass.as_ref(),
                hull: measurements.hull.as_ref(),
                physics_used,
            },
            &EntryCapture::for_input(input),
            archive,
        )
    }

    fn take_vehicle(
        &self,
        record: &Record<'_>,
        ready: bool,
    ) -> Result<Option<PendingVehicle>, CatalogError> {
        self.check_record(record)?;
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let mut state = self.lock()?;
        let pending = state
            .vehicles
            .pending
            .take()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        pending.subject.check_input(input)?;
        if pending.subject.key().map(RecordSubjectKey::index) != Some(record.index()) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        if state.error.is_some() || !ready {
            return Ok(None);
        }
        drop(state);
        if self
            .cancel
            .as_ref()
            .is_some_and(p4kx_extraction::CancelToken::is_cancelled)
        {
            return Err(CatalogError::Cancelled);
        }
        if pending.selected.is_some() != pending.totals_seen {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        Ok(Some(pending))
    }
}

fn bind_values(
    pending: &PendingVehicle,
    measurement: &MeasuredVehicle<'_>,
    capture: &EntryCapture<'_>,
    archive: &Archive,
) -> Result<ShipVehicleValues, CatalogError> {
    let MeasuredVehicle {
        kind_path,
        kind,
        mass,
        hull,
        physics_used,
    } = *measurement;
    let uses = capture.source_uses();
    let selected_inputs = vehicle_inputs(pending, capture, archive)?;
    let membership = pending
        .selected
        .as_ref()
        .map(|selected| selected.membership.origin(uses));
    let common = uses.origin_with_origins(
        std::iter::once(&selected_inputs).chain(membership.as_ref()),
        true,
    )?;
    let vehicle_kind = bind_kind(pending, kind_path, kind, uses, &selected_inputs)?;
    let mass_choice = pending.mass.choice.origin(uses);
    let hull_values = pending.hull.value.origin(uses);
    let mass_value = bind_mass(pending, mass, uses, &common, &mass_choice, physics_used)?;
    let mass_basis = match mass {
        Some(mass) => {
            let mut origins = vec![&common, &mass_choice];
            if physics_used {
                origins.push(&pending.operands.physics_selector);
            }
            uses.value_with_origins(SourceValue::Present(mass.basis), &origins)?
        }
        None => uses.value_with_origins(empty_basis(mass_value.state()), &[mass_value.origin()])?,
    };
    let hull_health = uses.value_with_origins(
        match hull {
            Some(hull) => SourceValue::Present(hull.health),
            None if unavailable_inputs(pending) => SourceValue::Unreadable,
            None if pending.hull.invalid_number => SourceValue::Invalid,
            None => SourceValue::Absent,
        },
        &[&common, &hull_values],
    )?;
    Ok(ShipVehicleValues {
        vehicle_kind,
        mass: mass_value,
        mass_basis,
        hull_health,
        physics_used,
    })
}

fn bind_kind(
    pending: &PendingVehicle,
    kind_path: VehicleKindPath,
    kind: Option<VehicleKind>,
    uses: &SourceUses,
    selected_inputs: &FieldOrigin,
) -> Result<SourcedValue<VehicleKind>, CatalogError> {
    if kind_path != pending.kind_path {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let value = kind.map_or(SourceValue::Absent, SourceValue::Present);
    let path = &pending.kind_path_origin;
    if pending.kind_path != VehicleKindPath::Space {
        return Ok(uses.value_with_origins(value, &[path])?);
    }
    let rolling = pending
        .selected
        .as_ref()
        .and_then(|selected| selected.rolling.as_ref())
        .map(|proof| proof.origin(uses));
    let mut origins = vec![path, selected_inputs];
    origins.extend(rolling.as_ref());
    Ok(uses.value_with_origins(value, &origins)?)
}

fn bind_mass(
    pending: &PendingVehicle,
    measured: Option<&Mass>,
    uses: &SourceUses,
    common: &FieldOrigin,
    mass_choice: &FieldOrigin,
    physics_used: bool,
) -> Result<SourcedValue<f64>, CatalogError> {
    let physics = &pending.operands.physics;
    if let Some(mass) = measured {
        let mass_values = pending.mass.value.origin(uses);
        let operands = match mass.basis {
            MassBasis::Parts if !physics_used && pending.totals_seen => &mass_values,
            MassBasis::Physics if physics_used => physics.origin(),
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(uses.value_with_origins(
            SourceValue::Present(mass.kg),
            &[common, mass_choice, operands],
        )?)
    } else {
        let mut state = if unavailable_inputs(pending) {
            SourceValue::Unreadable
        } else if physics_used {
            empty_mass(physics)
        } else {
            SourceValue::Absent
        };
        if matches!(state, SourceValue::Absent) && pending.mass.invalid_number {
            state = SourceValue::Invalid;
        }
        let mut origins = vec![common, mass_choice];
        if physics_used {
            origins.push(&pending.operands.physics_selector);
        }
        Ok(uses.value_with_origins(state, &origins)?)
    }
}

fn vehicle_inputs(
    pending: &PendingVehicle,
    capture: &EntryCapture<'_>,
    archive: &Archive,
) -> Result<FieldOrigin, CatalogError> {
    capture.check_archive(archive.input_origin())?;
    let uses = capture.source_uses();
    let needs_directory = pending
        .files
        .iter()
        .any(VehicleFileSelection::needs_directory);
    let directory = if needs_directory {
        uses.observe_root_directory_paths(archive)?
    } else {
        None
    };
    let directory_complete = crate::export_archive_health::inventory(archive).is_none();
    let mut attempts = Contributions::default();
    for file in &pending.files {
        attempts.add(file.attempt(capture, directory.as_ref(), directory_complete)?);
    }
    let attempts = attempts.origin(uses);
    let operands = &pending.operands;
    let mut origins = vec![operands.vehicle.origin(), &attempts];
    let mut complete = vehicle_capture::known(&operands.vehicle);
    let actual_caller = pending
        .selected
        .as_ref()
        .map(|selected| &selected.caller)
        .or_else(|| pending.files.first().map(VehicleFileSelection::caller));
    let caller;
    if let Some(actual) = actual_caller {
        if pending
            .files
            .iter()
            .any(|file| !file.caller().is_same_call(actual))
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let mut selected_caller = Contributions::default();
        selected_caller.add(actual.selection()?);
        caller = selected_caller.origin(uses);
        origins.extend([&caller, operands.modification.origin()]);
        complete &= vehicle_capture::known(&operands.modification);
    }
    if pending.selected.is_none() && !known_empty_vehicle(pending) {
        complete = false;
    }
    Ok(uses.origin_with_origins(origins, complete)?)
}

fn known_empty_vehicle(pending: &PendingVehicle) -> bool {
    let vehicle = &pending.operands.vehicle;
    vehicle_capture::known(vehicle) && vehicle.value() != Some(&true)
}

#[expect(
    clippy::match_same_arms,
    reason = "known source states remain explicit beside future non-exhaustive states"
)]
const fn empty_mass(physics: &SourcedValue<f32>) -> SourceValue<f64> {
    match physics.state() {
        SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null => {
            SourceValue::Absent
        }
        SourceValueState::Unreadable => SourceValue::Unreadable,
        SourceValueState::Unsupported => SourceValue::Unsupported,
        SourceValueState::Invalid => SourceValue::Invalid,
        _ => SourceValue::Unsupported,
    }
}

#[expect(
    clippy::match_same_arms,
    reason = "known source states remain explicit beside future non-exhaustive states"
)]
const fn empty_basis(state: SourceValueState) -> SourceValue<MassBasis> {
    match state {
        SourceValueState::Absent => SourceValue::Absent,
        SourceValueState::Null => SourceValue::Null,
        SourceValueState::Unreadable => SourceValue::Unreadable,
        SourceValueState::Invalid => SourceValue::Invalid,
        SourceValueState::Present | SourceValueState::Unsupported => SourceValue::Unsupported,
        _ => SourceValue::Unsupported,
    }
}

fn unavailable_inputs(pending: &PendingVehicle) -> bool {
    pending.selected.as_ref().map_or_else(
        || !known_empty_vehicle(pending),
        |selected| selected.unavailable_inputs,
    )
}
