// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::OriginalFlowFields;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::record_observation::RecordObservation;
use crate::selection::Kind;
use crate::template_field_capture::TemplateFieldCapture;

use super::{Input, Retention, eligibility, read, resource_links};

pub(crate) fn distribution<'f>(
    tree: &RecordTree<'f>,
    admission: &RecordObservation<'_, 'f>,
    input: &OriginalCatalogInput<'_, '_>,
    retention: Retention,
    role: ExportSourceRole,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Input<'f>, CatalogError> {
    let origins = input.origins();
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let found = read::distribution(tree, origins, cancel, &capture)?;
    crate::source_projection_notes::record(
        Kind::QualityDistributions,
        role,
        admission,
        tree,
        found.notes,
        ledger,
    )?;
    exclude(tree, Kind::QualityDistributions, retention, ledger);
    Ok(Input::Distribution {
        definition: Box::new(found.value),
        retention,
    })
}

pub(crate) fn quantization<'f>(
    tree: &RecordTree<'f>,
    admission: &RecordObservation<'_, 'f>,
    input: &OriginalCatalogInput<'_, '_>,
    retention: Retention,
    role: ExportSourceRole,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Input<'f>, CatalogError> {
    let origins = input.origins();
    let scope = input.record_scope(admission.record(), tree)?;
    let original = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&original, input, admission)?;
    let found = read::quantization(tree, origins, cancel, &capture)?;
    crate::source_projection_notes::record(
        Kind::QualityQuantizations,
        role,
        admission,
        tree,
        found.notes,
        ledger,
    )?;
    let eligible = quantization_eligibility(tree, origins, &original, role, cancel)?;
    exclude(tree, Kind::QualityQuantizations, retention, ledger);
    Ok(Input::Quantization {
        definition: Box::new(found.value),
        eligibility: eligible,
        retention,
    })
}

fn quantization_eligibility<'source>(
    tree: &RecordTree<'source>,
    origins: &CatalogOrigins,
    original: &OriginalFlowFields<'_, '_, 'source>,
    role: ExportSourceRole,
    cancel: Option<&CancelToken>,
) -> Result<eligibility::Eligibility, CatalogError> {
    let mut eligible = eligibility::read(tree, origins, original, cancel)?;
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
    Ok(eligible)
}

pub(crate) fn resources<'f>(
    tree: &RecordTree<'f>,
    input: &OriginalCatalogInput<'_, '_>,
    admission: &RecordObservation<'_, 'f>,
    cancel: Option<&CancelToken>,
) -> Result<Input<'f>, CatalogError> {
    resource_links::read(tree, input, admission, cancel).map(Input::Resource)
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
