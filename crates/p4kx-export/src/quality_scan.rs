// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::commodity_backfill_input::CommodityInputCapture;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::selection::Kind;

use super::{Input, Retention, eligibility, read, resource_links};

pub(crate) fn distribution<'f>(
    tree: &RecordTree<'f>,
    origins: &CatalogOrigins,
    retention: Retention,
    role: ExportSourceRole,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Input<'f>, CatalogError> {
    let found = read::distribution(tree, origins, cancel)?;
    crate::source_projection_notes::record(
        Kind::QualityDistributions,
        role,
        tree,
        found.notes,
        ledger,
    );
    exclude(tree, Kind::QualityDistributions, retention, ledger);
    Ok(Input::Distribution {
        definition: found.value,
        retention,
    })
}

pub(crate) fn quantization<'f>(
    tree: &RecordTree<'f>,
    origins: &CatalogOrigins,
    retention: Retention,
    role: ExportSourceRole,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Input<'f>, CatalogError> {
    let found = read::quantization(tree, origins, cancel)?;
    crate::source_projection_notes::record(
        Kind::QualityQuantizations,
        role,
        tree,
        found.notes,
        ledger,
    );
    let mut eligible = eligibility::read(tree, origins, cancel)?;
    let context = origins.record(tree.record_index());
    for field in [
        RecordHeaderInput::NormalizedPath,
        RecordHeaderInput::Field(RecordHeaderField::SourceType),
    ] {
        eligible.admission.header(&context, field, true)?;
    }
    if role == ExportSourceRole::Selected {
        eligible.admission.header(
            &context,
            RecordHeaderInput::Field(RecordHeaderField::RecordName),
            true,
        )?;
    }
    exclude(tree, Kind::QualityQuantizations, retention, ledger);
    Ok(Input::Quantization {
        definition: found.value,
        eligibility: eligible,
        retention,
    })
}

pub(crate) fn resources<'f>(
    tree: &RecordTree<'f>,
    origins: &CatalogOrigins,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Input<'f>, CatalogError> {
    let mut capture = CommodityInputCapture::new(tree.record_index());
    let found = resource_links::read(tree, origins, &mut capture, cancel)?;
    let captured = capture.finish();
    ledger.commodity_backfill_inputs.extend(captured.inputs);
    ledger.commodity_input_limits.extend(captured.limits);
    Ok(Input::Resource(found))
}

fn exclude(tree: &RecordTree<'_>, kind: Kind, retention: Retention, ledger: &mut ExportLedger) {
    use crate::catalog_row::{CatalogRow, CatalogRowExclusion, ExcludedCatalogRow};
    let reason = match retention {
        Retention::Selected => return,
        Retention::Dependency => CatalogRowExclusion::Filter,
        Retention::Leftover => {
            ledger.skipped_rows(kind, 1);
            CatalogRowExclusion::DevelopmentLeftover
        }
    };
    ledger.excluded_catalog_rows.push(ExcludedCatalogRow {
        row: CatalogRow {
            kind,
            guid: tree.guid(),
            record_path: tree.record_path().to_owned(),
        },
        reason,
    });
}
