// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use crate::ExportSourceRole;
use crate::catalog_number_health::NumberCaptures;
use crate::catalog_target_index::TargetIndex;
use crate::export_diagnostic::{ExportDiagnostic, ExportDiagnosticSeverity, FailedExportRecord};
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{ExportAccountingError, SourceObservation};
use crate::export_reference_issue::ExportUnresolvedReference;

/// Source inspections resolved onto each actual successful production read.
pub(crate) struct Fields {
    pub(crate) incomplete_kinds: BTreeSet<usize>,
    pub(crate) failed: Vec<FailedExportRecord>,
    pub(crate) diagnostics: Vec<ExportDiagnostic>,
    pub(crate) references: Vec<ExportUnresolvedReference>,
}

/// Reuses immutable canonical proofs by physical identity, without another decode or field walk.
pub(crate) fn resolve(
    inspections: Vec<FieldInspection>,
    sources: &[SourceObservation],
    rows: Option<&crate::catalog_row::CatalogRows>,
    excluded: &[crate::catalog_row::ExcludedCatalogRow],
) -> Result<Fields, ExportAccountingError> {
    let canonical = canonical_inspections(inspections)?;
    let targets = rows
        .map(|rows| TargetIndex::new(&rows.emitted, excluded, sources, &canonical))
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
        let inspection = canonical
            .get(&source.record_index)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        if inspection.guid != source.guid
            || source
                .record_path
                .as_deref()
                .is_some_and(|path| path != inspection.record_path)
        {
            return Err(ExportAccountingError::ConflictingFieldInspection);
        }
        if !inspection.number_requirements.is_empty() {
            let mut coverage = captured.for_record(source.record_index);
            if let Some(diagnostic) =
                crate::catalog_number_diagnostics::uncovered(inspection, source, &mut coverage)
            {
                retain_diagnostic(diagnostic, inspection, source, &mut fields);
            }
        }
        if !inspection.complete
            && let Some(kind) = source.kind
        {
            fields.incomplete_kinds.insert(kind.index());
        }
        retain(inspection, source, &mut fields);
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
    ));
    Ok(fields)
}

fn canonical_inspections(
    inspections: Vec<FieldInspection>,
) -> Result<BTreeMap<u32, FieldInspection>, ExportAccountingError> {
    let mut canonical = BTreeMap::<u32, FieldInspection>::new();
    for mut inspection in inspections {
        inspection.contextualize(None, ExportSourceRole::Dependency);
        match canonical.entry(inspection.record_index) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(inspection);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                if entry.get() != &inspection {
                    return Err(ExportAccountingError::ConflictingFieldInspection);
                }
            }
        }
    }
    Ok(canonical)
}

fn retain(inspection: &FieldInspection, source: &SourceObservation, fields: &mut Fields) {
    for original in &inspection.diagnostics {
        let mut diagnostic = original.clone();
        diagnostic.kind = source.kind;
        diagnostic.source_role = Some(source.role);
        retain_diagnostic(diagnostic, inspection, source, fields);
    }
    fields
        .references
        .extend(inspection.unresolved_references.iter().map(|original| {
            let mut reference = original.clone();
            reference.kind = source.kind;
            reference.source_role = source.role;
            reference
        }));
}

fn retain_diagnostic(
    diagnostic: ExportDiagnostic,
    inspection: &FieldInspection,
    source: &SourceObservation,
    fields: &mut Fields,
) {
    if diagnostic.severity != ExportDiagnosticSeverity::Information {
        fields.failed.push(FailedExportRecord {
            kind: source.kind,
            record_index: inspection.record_index,
            guid: inspection.guid.to_string(),
            record_path: Some(inspection.record_path.clone()),
            role: source.role,
            stage: diagnostic.stage,
            code: diagnostic.code,
        });
    }
    fields.diagnostics.push(diagnostic);
}

/// Retains every available blocking owner identity before aggregate example limits are applied.
pub(crate) fn failed_diagnostics<'diagnostic>(
    diagnostics: impl IntoIterator<Item = &'diagnostic ExportDiagnostic>,
) -> Vec<FailedExportRecord> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity != ExportDiagnosticSeverity::Information)
        .flat_map(|diagnostic| {
            diagnostic.examples.iter().filter_map(|example| {
                Some(FailedExportRecord {
                    kind: diagnostic.kind,
                    record_index: example.record_index?,
                    guid: example.record_guid.clone()?,
                    record_path: example.record_path.clone(),
                    role: diagnostic.source_role?,
                    stage: diagnostic.stage,
                    code: diagnostic.code,
                })
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "export_health_fields/numeric_capture_tests.rs"]
mod numeric_capture_tests;
