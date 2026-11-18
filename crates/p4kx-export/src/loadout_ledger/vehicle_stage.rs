// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceUse, SourceUses};

use crate::catalog_read::original::{
    OriginalDatum, RecordSubject, VehicleCaller, VehicleFileSelection,
};
use crate::catalog_v3::authored_fields::ship_source_read::ShipAggregateOperands;

/// Per-resolution staging, independent of cache-lifetime file proofs.
#[derive(Default)]
pub(super) struct VehicleCapture {
    pub(super) pending: Option<PendingVehicle>,
}

pub(super) struct PendingVehicle {
    pub(super) subject: RecordSubject,
    pub(super) operands: ShipAggregateOperands,
    pub(super) files: Vec<VehicleFileSelection>,
    pub(super) selected: Option<SelectedVehicle>,
    pub(super) mass: MetricContributions,
    pub(super) hull: MetricContributions,
    pub(super) totals_seen: bool,
}

pub(super) struct SelectedVehicle {
    pub(super) caller: VehicleCaller,
    pub(super) membership: Contributions,
    pub(super) unavailable_inputs: bool,
}

#[derive(Default)]
pub(super) struct MetricContributions {
    pub(super) value: Contributions,
    pub(super) choice: Contributions,
    pub(super) invalid_number: bool,
}

pub(super) struct Contributions {
    uses: Vec<SourceUse>,
    complete: bool,
}

impl Default for Contributions {
    fn default() -> Self {
        Self {
            uses: Vec::new(),
            complete: true,
        }
    }
}

impl Contributions {
    pub(super) fn add(&mut self, datum: OriginalDatum<()>) {
        let (_, uses, complete) = datum.into_parts();
        self.uses.extend(uses);
        self.complete &= complete;
    }

    pub(super) fn extend(&mut self, other: Self) {
        self.uses.extend(other.uses);
        self.complete &= other.complete;
    }

    pub(super) const fn incomplete(&mut self) {
        self.complete = false;
    }

    pub(super) fn origin(&self, sources: &SourceUses) -> FieldOrigin {
        sources.origin(&self.uses, self.complete)
    }
}
