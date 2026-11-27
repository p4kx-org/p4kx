// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{Archive, CancelToken, SourceValueState};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::contract::Ship;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::{DerivedFailure, ExportLedger, SourceDiagnosis};
use crate::record_derived_observation::DerivedOperation;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind, VariantCounts};

/// Records the canonical physical Ship decisions and their failed derived outputs.
pub(crate) fn project_for_catalog<'source>(
    archive: &'source Archive,
    input: &'source OriginalCatalogInput<'source, 'source>,
    rows: &mut [Ship<'source>],
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<VariantCounts, CatalogError> {
    let mut survey = super::read(
        archive,
        input,
        rows.iter()
            .map(|row| (row.source_record_index, &row.vehicle_definition)),
        cancel,
    )?;
    let mut groups = HashSet::new();
    let mut count = 0;
    for row in rows {
        checkpoint(cancel)?;
        let decision = survey
            .decisions
            .remove(&row.source_record_index)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        if let Some(target) = decision.value() {
            let group = target
                .target_record_index()
                .value()
                .copied()
                .ok_or(ExportAccountingError::MissingFieldInspection)?;
            groups.insert(group);
            count += 1;
        }
        row.variant_of = decision;
        if row.variant_of.expanded().state() == SourceValueState::Unreadable {
            retain_failure(input, row, ledger)?;
        }
    }
    Ok(VariantCounts {
        groups: groups.len(),
        variants: count,
    })
}

fn retain_failure(
    input: &OriginalCatalogInput<'_, '_>,
    row: &Ship<'_>,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    let index = row.source_record_index;
    let record = input
        .forge()
        .record(index)
        .ok_or(ExportAccountingError::MissingPhysicalReferenceTarget { index })?;
    let observation = RecordObservation::capture(input, &record)?;
    let subject = observation.subject().clone();
    let excerpt = ExampleBodyReceipt::technical_with_evidence(
        input,
        subject.clone(),
        "variantOf",
        row.variant_of.origin(),
        row.variant_of.failed_source_uses(),
    )?;
    let diagnosis =
        SourceDiagnosis::from_body(ExportDiagnosticCode::DerivedProjectionFailure, excerpt)?;
    ledger.derived_failure(DerivedFailure::of_operation(
        subject,
        Kind::Ships,
        Kind::Ships,
        ExportSourceRole::Selected,
        DerivedOperation::ShipVariant,
        diagnosis,
    )?);
    Ok(())
}
