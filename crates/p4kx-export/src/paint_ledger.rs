// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::Kind;
use crate::contract::Item;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{DerivedFailure, ExportLedger, SourceDiagnosis, SourceDisposition};
use crate::paint_port::VehicleFacts;

#[cfg(test)]
mod tests;

/// Captures actual paint inputs from the original selected physical sources and their read proofs.
pub(crate) fn inputs(
    source: &ExportLedger,
    candidates: &[&Item<'_>],
    vehicles: &[VehicleFacts],
) -> Result<ExportLedger, crate::CatalogError> {
    let paints: BTreeSet<_> = candidates
        .iter()
        .map(|item| item.source_record_index)
        .collect();
    let ships: BTreeSet<_> = vehicles
        .iter()
        .map(|vehicle| {
            vehicle
                .source_record_index
                .ok_or(crate::ExportAccountingError::MissingFieldInspection)
        })
        .collect::<Result<_, _>>()?;
    let unreadable: BTreeSet<_> = vehicles
        .iter()
        .filter(|vehicle| vehicle.implementation.unreadable.is_some())
        .filter_map(|vehicle| vehicle.source_record_index)
        .collect();
    let mut ledger = ExportLedger::default();
    ledger.inventory(Kind::Paints);
    vehicle_entries(source, &mut ledger, !paints.is_empty());
    for observation in &source.sources {
        if observation.role != ExportSourceRole::Selected
            || observation.disposition != SourceDisposition::Projected
        {
            continue;
        }
        let role = match observation.kind {
            Some(Kind::Items) if paints.contains(&observation.record_index) => {
                ExportSourceRole::DerivedInput
            }
            Some(Kind::Ships)
                if ships.contains(&observation.record_index) && !paints.is_empty() =>
            {
                ExportSourceRole::Dependency
            }
            _ => continue,
        };
        let mut input = observation.clone();
        input.kind = Some(Kind::Paints);
        input.role = role;
        ledger.observe(input);
        if role == ExportSourceRole::Dependency && unreadable.contains(&observation.record_index) {
            ledger.incomplete_inventory(Kind::Paints);
            ledger.derived_failure(DerivedFailure::of_source(
                observation,
                Kind::Paints,
                Kind::Paints,
                role,
                SourceDiagnosis::default(),
            )?);
        }
    }
    Ok(ledger)
}

fn vehicle_entries(source: &ExportLedger, target: &mut ExportLedger, needed: bool) {
    use crate::EntryOperation::{
        VehicleImplementation, VehicleImplementationBytes, VehiclePatch, VehiclePatchBytes,
    };

    if !needed {
        return;
    }
    let required = source.entries.iter().filter(|entry| {
        entry.kind == Kind::Ships
            && matches!(
                entry.operation,
                VehicleImplementation
                    | VehicleImplementationBytes
                    | VehiclePatch
                    | VehiclePatchBytes
            )
    });
    for entry in required {
        let mut dependency = entry.clone();
        dependency.kind = Kind::Paints;
        dependency.role = ExportSourceRole::Dependency;
        if dependency.disposition.failed() {
            target.incomplete_inventory(Kind::Paints);
        }
        target.entry(dependency);
    }
}
