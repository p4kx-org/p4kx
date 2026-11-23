// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, SourceUseRef, SourceValueState, SourcedValue,
};
use serde::Serialize;

use super::{ShipBase, ShipBaseError, VehicleKind};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::loadout::ledger::ShipAggregateValues;
use crate::{CatalogError, ExportAccountingError, Playability};

/// Physical identity and observed field evidence of one standalone vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipBaseEvidence {
    record_index: u32,
    vehicle_kind_state: SourceValueState,
    playability: SourcedValue<Playability>,
    field_origins: FieldOrigins,
}

impl ShipBaseEvidence {
    pub(crate) fn collect(
        ship: &ShipBase,
        header: &CanonicalRecordHeader<'_>,
        kind: &SourcedValue<VehicleKind>,
        playability: SourcedValue<Playability>,
        values: &ShipAggregateValues,
    ) -> Result<Self, ShipBaseError> {
        if header.guid().value() != Some(&ship.guid)
            || header.class_name().value().copied() != Some(ship.class_name.as_str())
            || header.record_path().value().copied() != Some(ship.record_path.as_str())
            || kind.value().copied() != ship.vehicle_kind
        {
            return Err(CatalogError::from(ExportAccountingError::ConflictingSourceDatum).into());
        }
        let record_index = header
            .record_index()
            .value()
            .copied()
            .ok_or_else(|| CatalogError::from(ExportAccountingError::MissingFieldInspection))?;
        let mut field_origins = FieldOrigins::new();
        for (path, origin) in [
            ("/recordIndex", header.record_index().origin()),
            ("/guid", header.guid.origin()),
            ("/className", header.class_name().origin()),
            ("/recordPath", header.record_path().origin()),
            ("/vehicleKind", kind.origin()),
            ("/vehicleKindState", kind.origin()),
        ] {
            insert(&mut field_origins, path, origin)?;
        }
        aggregate_origins(ship, values, &mut field_origins)?;
        Ok(Self {
            record_index,
            vehicle_kind_state: kind.state(),
            playability,
            field_origins,
        })
    }

    /// Returns the original physical position within the originating Build.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// Returns the observed result state of the existing nullable vehicle kind.
    #[must_use]
    pub const fn vehicle_kind_state(&self) -> SourceValueState {
        self.vehicle_kind_state
    }

    /// Borrows the canonical sourced playability decision.
    #[must_use]
    pub const fn playability(&self) -> &SourcedValue<Playability> {
        &self.playability
    }

    /// Borrows the origins at exact paths relative to this standalone row.
    #[must_use]
    pub const fn field_origins(&self) -> &FieldOrigins {
        &self.field_origins
    }

    pub(super) fn references(&self) -> impl Iterator<Item = &SourceUseRef> {
        self.field_origins
            .iter()
            .map(|(_, origin)| origin)
            .chain(std::iter::once(self.playability.origin()))
            .flat_map(FieldOrigin::source_uses_refs)
    }
}

fn aggregate_origins(
    ship: &ShipBase,
    values: &ShipAggregateValues,
    fields: &mut FieldOrigins,
) -> Result<(), ShipBaseError> {
    let grids_with_issue = u64::try_from(ship.cargo.grids_with_issue)
        .map_err(|_| CatalogError::from(ExportAccountingError::CounterOverflow))?;
    if ship.mass.as_ref().map(|mass| mass.kg) != values.mass.value().copied()
        || ship.mass.as_ref().map(|mass| mass.basis) != values.mass_basis.value().copied()
        || ship.hull.as_ref().map(|hull| hull.health) != values.hull_health.value().copied()
        || ship.cargo.scu != values.cargo.scu().value().copied()
        || values.cargo.grids_with_issue().value() != Some(&grids_with_issue)
    {
        return Err(CatalogError::from(ExportAccountingError::ConflictingSourceDatum).into());
    }
    if ship.mass.is_some() {
        insert(fields, "/mass/kg", values.mass.origin())?;
        insert(fields, "/mass/basis", values.mass_basis.origin())?;
    } else {
        insert(fields, "/mass", values.mass.origin())?;
    }
    let hull = if ship.hull.is_some() {
        "/hull/health"
    } else {
        "/hull"
    };
    insert(fields, hull, values.hull_health.origin())?;
    insert(fields, "/cargo/scu", values.cargo.scu().origin())?;
    insert(
        fields,
        "/cargo/gridsWithIssue",
        values.cargo.grids_with_issue().origin(),
    )
}

fn insert(
    fields: &mut FieldOrigins,
    path: &str,
    origin: &FieldOrigin,
) -> Result<(), ShipBaseError> {
    Ok(fields.insert(FieldPath::parse(path)?, origin.clone())?)
}
