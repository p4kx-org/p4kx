// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::ship::ShipSource;
use crate::variant_use_sets::VariantSourceUseSets;
use crate::{CatalogError, ExportAccountingError, ship_variant_survey};

pub(super) fn project<'source>(
    archive: &'source Archive,
    input: &'source OriginalCatalogInput<'source, 'source>,
    rows: &mut [ShipSource<'source>],
    coverage: &mut CatalogRecordCoverage,
    cancel: Option<&CancelToken>,
) -> Result<VariantSourceUseSets, CatalogError> {
    let mut requests = Vec::new();
    for row in rows.iter() {
        checkpoint(cancel)?;
        requests.push((index(row)?, row.vehicle_definition()));
    }
    let mut result = ship_variant_survey::read(archive, input, requests, cancel)?;
    for row in rows.iter_mut() {
        checkpoint(cancel)?;
        let physical = index(row)?;
        let decision = result
            .decisions
            .remove(&physical)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        if !decision.origin().source_set_complete() {
            coverage.incomplete_fields.push(physical);
        }
        row.set_variant(decision);
    }
    coverage.incomplete_fields.sort_unstable();
    coverage.incomplete_fields.dedup();
    VariantSourceUseSets::from_decisions(rows.iter().map(ShipSource::variant_decision))
}

fn index(row: &ShipSource<'_>) -> Result<u32, CatalogError> {
    row.source()
        .record_index()
        .value()
        .copied()
        .ok_or_else(|| ExportAccountingError::MissingFieldInspection.into())
}
