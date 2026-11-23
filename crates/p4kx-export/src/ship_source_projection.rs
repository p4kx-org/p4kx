// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourceEvidenceError};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::authored_fields::ship_source_read::{self, ShipSourceRead};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::ShipAggregateValues;
use crate::record_observation::RecordObservation;
use crate::ship_base::ShipRead;
use crate::ship_pool::ShipPool;
use crate::{CatalogError, CatalogReadFailure, ExportAccountingError, Kind};

#[cfg(test)]
#[path = "ship_source_projection_direct.rs"]
pub(crate) mod direct;

pub(crate) struct ProjectedShipSource<'read, 'source> {
    pub(crate) source: ShipSourceRead<'read, 'source>,
    pub(crate) read: Result<ShipRead, CatalogReadFailure<'source>>,
    pub(crate) aggregates: Option<ShipAggregateValues>,
}

/// Shares original source, resolver captures and conditional Physics diagnostics.
pub(crate) fn read<'read, 'source>(
    pool: &ShipPool<'source>,
    projection: &'read CatalogProjection<'_, 'source>,
    input: &'source OriginalCatalogInput<'source, 'source>,
    admission: &'read RecordObservation<'_, 'source>,
    tree: &'read RecordTree<'source>,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<ProjectedShipSource<'read, 'source>, CatalogError> {
    if !std::ptr::eq(projection.original(), input) {
        return Err(SourceEvidenceError::ForeignEvidence.into());
    }
    let mut source = ship_source_read::read(tree, projection, admission, cancel)?;
    let read = pool.read_catalog(
        &source.aggregate,
        admission.record(),
        tree,
        ledger,
        input,
        cancel,
    )?;
    ledger.merge(read.ledger);
    if read.read.is_ok() {
        let aggregates = read
            .aggregates
            .as_ref()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        record_notes(
            &mut source,
            admission,
            tree,
            aggregates.vehicle.physics_used,
            ledger,
        )?;
    }
    Ok(ProjectedShipSource {
        source,
        read: read.read,
        aggregates: read.aggregates,
    })
}

fn record_notes(
    source: &mut ShipSourceRead<'_, '_>,
    admission: &RecordObservation<'_, '_>,
    tree: &RecordTree<'_>,
    physics_used: bool,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    if physics_used {
        source.notes.append(&mut source.physics_notes);
    }
    crate::source_projection_notes::record(
        Kind::Ships,
        ExportSourceRole::Selected,
        admission,
        tree,
        std::mem::take(&mut source.notes),
        ledger,
    )
}
