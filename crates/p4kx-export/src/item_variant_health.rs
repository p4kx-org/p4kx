// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, SourceValueState};

use super::{index, project};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::{DerivedFailure, ExportLedger, SourceDiagnosis};
use crate::item::Item;
use crate::record_derived_observation::DerivedOperation;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind, VariantCounts};

/// Projects Item variants and accounts for undecidable decisions on emitted Items.
pub(crate) fn project_for_catalog<'source>(
    input: &OriginalCatalogInput<'source, 'source>,
    items: &mut [Item<'source>],
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<VariantCounts, CatalogError> {
    let counts = project(input, items, cancel)?;
    for item in items {
        checkpoint(cancel)?;
        if item.variant_of().state() == SourceValueState::Unreadable {
            retain_failure(input, item, ledger)?;
        }
    }
    Ok(counts)
}

fn retain_failure(
    input: &OriginalCatalogInput<'_, '_>,
    item: &Item<'_>,
    ledger: &mut ExportLedger,
) -> Result<(), CatalogError> {
    let index = index(item)?;
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
        item.variant_of().origin(),
        item.variant_failed_source_uses(),
    )?;
    let diagnosis =
        SourceDiagnosis::from_body(ExportDiagnosticCode::DerivedProjectionFailure, excerpt)?;
    ledger.derived_failure(DerivedFailure::of_operation(
        subject,
        Kind::Items,
        Kind::Items,
        ExportSourceRole::Selected,
        DerivedOperation::ItemVariant,
        diagnosis,
    )?);
    Ok(())
}
