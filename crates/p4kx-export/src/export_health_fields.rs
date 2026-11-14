// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::catalog_number_health::NumberCaptures;
use crate::catalog_read::original::ReferenceObservation;
use crate::catalog_target_index::TargetIndex;
use crate::export_diagnostic::ExportDiagnosticSeverity;
use crate::export_field_inspection::{FieldInspection, FieldInspections};
use crate::export_ledger::{ExportAccountingError, SourceObservation};
use crate::health_diagnostic_observation::DiagnosticObservation as ExportDiagnostic;
use crate::health_failed_observation::FailedRecordObservation as FailedExportRecord;

/// Source inspections resolved onto each actual successful production read.
pub(crate) struct Fields {
    pub(crate) incomplete_kinds: BTreeSet<usize>,
    pub(crate) failed: Vec<FailedExportRecord>,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
    pub(crate) references: Vec<ReferenceObservation>,
}

/// Reuses immutable canonical proofs by physical identity, without another decode or field walk.
pub(crate) fn resolve(
    inspections: Vec<FieldInspection>,
    sources: &[SourceObservation],
    rows: Option<&crate::catalog_row::CatalogRows>,
    excluded: &[crate::catalog_row::ExcludedCatalogRow],
) -> Result<Fields, ExportAccountingError> {
    let canonical = FieldInspections::index(inspections)?;
    let targets = rows
        .map(|rows| TargetIndex::new(&rows.emitted, excluded, sources, canonical.whole()))
        .transpose()?;
    let captured = NumberCaptures::new(rows, targets.as_ref());
    let mut fields = Fields {
        incomplete_kinds: BTreeSet::new(),
        failed: Vec::new(),
        diagnostics: Vec::new(),
        references: Vec::new(),
    };
    for source in sources
        .iter()
        .filter(|source| source.disposition.read_succeeded())
    {
        let inspection = canonical.for_source(source)?;
        if inspection.guid != source.guid
            || source
                .record_path
                .as_ref()
                .map(crate::HealthText::as_str)
                .is_some_and(|path| path != inspection.record_path.as_str())
        {
            return Err(ExportAccountingError::ConflictingFieldInspection);
        }
        source.key()?;
        let mut owner = inspection.subject.clone();
        owner.merge(source.subject().clone())?;
        if !inspection.number_requirements.is_empty() {
            let mut coverage = captured.for_source(source)?;
            if let Some(diagnostic) =
                crate::catalog_number_diagnostics::uncovered(inspection, source, &mut coverage)?
            {
                retain_diagnostic(diagnostic, inspection, source, &mut fields)?;
            }
        }
        if !inspection.complete
            && let Some(kind) = source.kind
        {
            fields.incomplete_kinds.insert(kind.index());
        }
        retain(inspection, source, &mut fields)?;
    }
    let reference_diagnostics = fields.diagnostics.len();
    crate::catalog_reference_health::resolve(
        &canonical,
        sources,
        rows,
        targets.as_ref(),
        &mut fields,
    )?;
    fields.failed.extend(failed_diagnostics(
        fields.diagnostics.iter().skip(reference_diagnostics),
    )?);
    Ok(fields)
}

fn retain(
    inspection: &FieldInspection,
    source: &SourceObservation,
    fields: &mut Fields,
) -> Result<(), ExportAccountingError> {
    for original in &inspection.diagnostics {
        let mut diagnostic = original.clone();
        diagnostic.kind = source.kind;
        diagnostic.source_role = Some(source.role);
        retain_diagnostic(diagnostic, inspection, source, fields)?;
    }
    fields
        .references
        .extend(inspection.unresolved_references.iter().map(|original| {
            let mut reference = original.clone();
            reference.contextualize(source.kind, source.role);
            reference
        }));
    Ok(())
}

fn retain_diagnostic(
    diagnostic: ExportDiagnostic,
    inspection: &FieldInspection,
    source: &SourceObservation,
    fields: &mut Fields,
) -> Result<(), ExportAccountingError> {
    if diagnostic.severity != ExportDiagnosticSeverity::Information {
        let mut owner = inspection.subject.clone();
        owner.merge(source.subject().clone())?;
        fields.failed.push(FailedExportRecord::new(
            source.kind,
            owner,
            source.role,
            diagnostic.stage,
            diagnostic.code,
        )?);
    }
    fields.diagnostics.push(diagnostic);
    Ok(())
}

/// Retains every available blocking owner identity before aggregate example limits are applied.
pub(crate) fn failed_diagnostics<'diagnostic>(
    diagnostics: impl IntoIterator<Item = &'diagnostic ExportDiagnostic>,
) -> Result<Vec<FailedExportRecord>, ExportAccountingError> {
    let mut failed = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.severity == ExportDiagnosticSeverity::Information {
            continue;
        }
        let Some(role) = diagnostic.source_role else {
            continue;
        };
        for example in &diagnostic.examples {
            if example.subject().key().is_none() {
                continue;
            }
            failed.push(FailedExportRecord::new(
                diagnostic.kind,
                example.subject().clone(),
                role,
                diagnostic.stage,
                diagnostic.code,
            )?);
        }
    }
    Ok(failed)
}
#[cfg(test)]
#[path = "export_health_fields/numeric_capture_tests.rs"]
mod numeric_capture_tests;
