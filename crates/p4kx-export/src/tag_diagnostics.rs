// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::TagRelationIssue;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_ledger::ExportLedger;
use crate::health_diagnostic_observation::{DiagnosticObservation, ExampleObservation};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

pub(super) fn relation(
    ledger: &mut ExportLedger,
    admission: &RecordObservation<'_, '_>,
    reason: TagRelationIssue,
    body: ExampleBodyReceipt,
) -> Result<(), CatalogError> {
    let label = match reason {
        TagRelationIssue::Dangling => "tag_relation_dangling",
        TagRelationIssue::Repeated => "tag_relation_repeated",
        TagRelationIssue::MalformedValue => "tag_relation_malformed",
    };
    let (field, source_type, target) = body.description()?;
    let example = ExampleObservation::captured(
        admission.subject().clone(),
        field.as_deref(),
        source_type.as_deref(),
        target,
        body,
    )?;
    issue(ledger, admission, label, example);
    Ok(())
}
pub(super) fn duplicate(
    input: &OriginalCatalogInput<'_, '_>,
    ledger: &mut ExportLedger,
    admission: &RecordObservation<'_, '_>,
) -> Result<(), CatalogError> {
    let subject = admission.subject().clone();
    let body = ExampleBodyReceipt::duplicate_guid_header(input, subject.clone())?;
    let (field, source_type, target_guid) = body.description()?;
    let example = ExampleObservation::captured(
        subject,
        field.as_deref(),
        source_type.as_deref(),
        target_guid,
        body,
    )?;
    issue(ledger, admission, "tag_duplicate_guid", example);
    Ok(())
}

fn issue(
    ledger: &mut ExportLedger,
    admission: &RecordObservation<'_, '_>,
    label: &str,
    retained: ExampleObservation,
) {
    ledger.field_diagnostic(DiagnosticObservation {
        kind: Some(Kind::Tags),
        stage: ExportDiagnosticStage::DerivedProjection,
        code: ExportDiagnosticCode::SourceWarning,
        severity: ExportDiagnosticSeverity::Information,
        source_role: Some(
            if admission
                .record()
                .type_name()
                .is_ok_and(|name| name == "TagDatabase")
            {
                ExportSourceRole::Dependency
            } else {
                ExportSourceRole::Selected
            },
        ),
        warning_code: Some(label.to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Occurrences,
        examples: vec![retained],
    });
}
