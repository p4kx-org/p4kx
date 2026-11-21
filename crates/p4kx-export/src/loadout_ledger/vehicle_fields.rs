// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    VehicleImplementation, VehicleTotalFieldPresence, VehicleTotalFieldUse, VehicleTotalMetric,
    VehicleTotals, VehicleXmlSite,
};
use p4kx_extraction::SourceUseRole;

use super::vehicle_stage::{Contributions, MetricContributions, PendingVehicle};
use super::{Capture, State};
use crate::catalog_read::original::{EntryCapture, RecordSubjectKey};
use crate::{CatalogError, ExportAccountingError};

impl Capture<'_> {
    pub(crate) fn vehicle_totals(
        &self,
        record_index: u32,
        vehicle: &VehicleImplementation,
    ) -> VehicleTotals {
        let start = (|| {
            let mut state = self.lock()?;
            let pending = state
                .vehicles
                .pending
                .as_mut()
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            if pending.subject.key().map(RecordSubjectKey::index) != Some(record_index)
                || pending.selected.is_none()
                || pending.totals_seen
            {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            pending.totals_seen = true;
            drop(state);
            Ok::<_, CatalogError>(())
        })();
        if let Err(error) = start {
            self.record_error(error);
        }
        let totals = vehicle.totals_with_field_uses(|field| {
            if self.stopped() {
                return;
            }
            if let Err(error) = self.total_field(record_index, field) {
                self.record_error(error);
            }
        });
        if !totals.unreadable().is_empty()
            && !self.stopped()
            && let Err(error) = self.record_invalid_numbers(record_index, &totals)
        {
            self.record_error(error);
        }
        totals
    }

    fn record_invalid_numbers(
        &self,
        record_index: u32,
        totals: &VehicleTotals,
    ) -> Result<(), CatalogError> {
        let mut state = self.lock()?;
        let pending = state
            .vehicles
            .pending
            .as_mut()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if pending.subject.key().map(RecordSubjectKey::index) != Some(record_index)
            || !pending.totals_seen
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        for rejected in totals.unreadable() {
            match rejected.attribute() {
                "mass" => pending.mass.invalid_number = true,
                "damageMax" => pending.hull.invalid_number = true,
                _ => (),
            }
        }
        drop(state);
        Ok(())
    }

    fn total_field(
        &self,
        record_index: u32,
        field: VehicleTotalFieldUse<'_>,
    ) -> Result<(), CatalogError> {
        let mass = match field.metric() {
            VehicleTotalMetric::Mass => true,
            VehicleTotalMetric::HullHealth => false,
            _ => return Ok(()),
        };
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let mut state = self.lock()?;
        let pending = state
            .vehicles
            .pending
            .as_ref()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if pending.subject.key().map(RecordSubjectKey::index) != Some(record_index)
            || !pending.totals_seen
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let contribution =
            field_contributions(&state, pending, &EntryCapture::for_input(input), field)?;
        let pending = state
            .vehicles
            .pending
            .as_mut()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        let target = if mass {
            &mut pending.mass
        } else {
            &mut pending.hull
        };
        target.value.extend(contribution.value);
        target.choice.extend(contribution.choice);
        drop(state);
        Ok(())
    }
}

fn field_contributions(
    state: &State,
    pending: &PendingVehicle,
    entries: &EntryCapture<'_>,
    field: VehicleTotalFieldUse<'_>,
) -> Result<MetricContributions, CatalogError> {
    let supported = matches!(
        field.presence(),
        VehicleTotalFieldPresence::Present | VehicleTotalFieldPresence::Absent
    );
    let (role, role_known) = match field.role() {
        role @ (SourceUseRole::Conversion | SourceUseRole::Selector) => (role, true),
        _ => (SourceUseRole::Selector, false),
    };
    if matches!(field.presence(), VehicleTotalFieldPresence::Absent)
        && role == SourceUseRole::Conversion
    {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let mut contribution = MetricContributions::default();
    if let Some(site) = field.site() {
        add_observed_site(state, pending, entries, site, role, &mut contribution)?;
    } else {
        contribution.value.incomplete();
        contribution.choice.incomplete();
    }
    if let Some(evidence) = field.evidence() {
        for selector in evidence.selectors() {
            add_observed_site(
                state,
                pending,
                entries,
                selector,
                SourceUseRole::Selector,
                &mut contribution,
            )?;
        }
    }
    if !supported || !role_known {
        contribution.value.incomplete();
        contribution.choice.incomplete();
    }
    Ok(contribution)
}

fn add_observed_site(
    state: &State,
    pending: &PendingVehicle,
    entries: &EntryCapture<'_>,
    site: &VehicleXmlSite,
    role: SourceUseRole,
    contribution: &mut MetricContributions,
) -> Result<(), CatalogError> {
    if !state.files.contains(site.file()) {
        return Err(ExportAccountingError::UnobservedFailure.into());
    }
    add_site(pending, entries, site, role, &mut contribution.value)?;
    add_site(
        pending,
        entries,
        site,
        SourceUseRole::Selector,
        &mut contribution.choice,
    )
}

fn add_site(
    pending: &PendingVehicle,
    entries: &EntryCapture<'_>,
    site: &VehicleXmlSite,
    role: SourceUseRole,
    target: &mut Contributions,
) -> Result<(), CatalogError> {
    let caller = &pending
        .selected
        .as_ref()
        .ok_or(ExportAccountingError::UnobservedFailure)?
        .caller;
    let file = pending
        .files
        .iter()
        .find(|file| file.matches(site.file(), caller))
        .ok_or(ExportAccountingError::UnobservedFailure)?;
    target.add(file.site(entries, site, role)?);
    Ok(())
}
