// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, FieldOrigin, Record, SourceUses, SourceValue, SourceValueState, SourcedValue,
};

use super::Capture;
use super::vehicle_capture;
use super::vehicle_stage::{Contributions, PendingVehicle};
use crate::catalog_read::original::{EntryCapture, RecordSubjectKey, VehicleFileSelection};
use crate::ship_base::{ShipRead, VehicleKindPath};
use crate::{CatalogError, ExportAccountingError, MassBasis, VehicleKind};

/// The actual `ShipReader` aggregates and their immediate source sets.
pub(crate) struct ShipAggregateValues {
    pub(crate) vehicle_kind: SourcedValue<VehicleKind>,
    pub(crate) mass: SourcedValue<f64>,
    pub(crate) mass_basis: SourcedValue<MassBasis>,
    pub(crate) hull_health: SourcedValue<f64>,
    pub(crate) physics_used: bool,
}

impl Capture<'_> {
    pub(crate) fn finish_vehicle(
        &self,
        archive: &Archive,
        record: &Record<'_>,
        read: Option<&ShipRead>,
        physics_used: bool,
    ) -> Result<Option<ShipAggregateValues>, CatalogError> {
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
        if state.error.is_some() || read.is_none() {
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
        let read = read.ok_or(ExportAccountingError::UnobservedFailure)?;
        if pending.selected.is_some() != pending.totals_seen {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        Ok(Some(bind_values(
            &pending,
            read,
            &EntryCapture::for_input(input),
            archive,
            physics_used,
        )?))
    }
}

fn bind_values(
    pending: &PendingVehicle,
    read: &ShipRead,
    capture: &EntryCapture<'_>,
    archive: &Archive,
    physics_used: bool,
) -> Result<ShipAggregateValues, CatalogError> {
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
    let vehicle_kind = bind_kind(pending, read, uses, &selected_inputs)?;
    let mass_choice = pending.mass.choice.origin(uses);
    let hull_values = pending.hull.value.origin(uses);
    let mass = bind_mass(pending, read, uses, &common, &mass_choice, physics_used)?;
    let mass_basis = match &read.base.mass {
        Some(mass) => {
            let mut origins = vec![&common, &mass_choice];
            if physics_used {
                origins.push(&pending.operands.physics_selector);
            }
            uses.value_with_origins(SourceValue::Present(mass.basis), &origins)?
        }
        None => uses.value_with_origins(empty_basis(mass.state()), &[mass.origin()])?,
    };
    let hull_health = uses.value_with_origins(
        match &read.base.hull {
            Some(hull) => SourceValue::Present(hull.health),
            None if unavailable_inputs(pending) => SourceValue::Unreadable,
            None if pending.hull.invalid_number => SourceValue::Invalid,
            None => SourceValue::Absent,
        },
        &[&common, &hull_values],
    )?;
    Ok(ShipAggregateValues {
        vehicle_kind,
        mass,
        mass_basis,
        hull_health,
        physics_used,
    })
}

fn bind_kind(
    pending: &PendingVehicle,
    read: &ShipRead,
    uses: &SourceUses,
    selected_inputs: &FieldOrigin,
) -> Result<SourcedValue<VehicleKind>, CatalogError> {
    if VehicleKindPath::of(&read.base.record_path) != pending.kind_path {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let value = read
        .base
        .vehicle_kind
        .map_or(SourceValue::Absent, SourceValue::Present);
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
    read: &ShipRead,
    uses: &SourceUses,
    common: &FieldOrigin,
    mass_choice: &FieldOrigin,
    physics_used: bool,
) -> Result<SourcedValue<f64>, CatalogError> {
    let physics = &pending.operands.physics;
    if let Some(mass) = &read.base.mass {
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
