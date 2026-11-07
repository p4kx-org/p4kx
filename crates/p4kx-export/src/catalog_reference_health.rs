// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::catalog_reference;
use crate::catalog_reference::CatalogReference;
use crate::catalog_row::{CatalogRowExclusion, CatalogRows, ExcludedCatalogRow};
use crate::catalog_target_index::TargetIndex;
use crate::export_diagnostic::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole,
};
use crate::export_field_inspection::FieldInspection;
use crate::export_health_fields::Fields;
use crate::export_ledger::{ExportAccountingError, SourceDisposition, SourceObservation};
use crate::export_reference_issue::{ExportReferenceStatus, ExportUnresolvedReference};

/// Resolves captured source links using final emitted rows and exact physical source outcomes.
pub(crate) fn resolve(
    canonical: &BTreeMap<u32, FieldInspection>,
    sources: &[SourceObservation],
    rows: Option<&CatalogRows>,
    excluded: &[ExcludedCatalogRow],
    fields: &mut Fields,
) -> Result<(), ExportAccountingError> {
    let Some(rows) = rows else {
        if canonical
            .values()
            .any(|proof| !proof.catalog_references.is_empty())
        {
            return Err(ExportAccountingError::MissingCatalogRows);
        }
        return Ok(());
    };
    let targets = TargetIndex::new(&rows.emitted, excluded, sources, canonical)?;
    ambiguous_rows(&targets, canonical, fields)?;
    let mut by_target = BTreeMap::<u32, Vec<&SourceObservation>>::new();
    for source in sources {
        by_target
            .entry(source.record_index)
            .or_default()
            .push(source);
    }
    for source in sources
        .iter()
        .filter(|source| source.disposition.read_succeeded())
    {
        let proof = canonical
            .get(&source.record_index)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        for reference in &proof.catalog_references {
            let target_sources = by_target
                .get(&reference.target_record_index)
                .map_or(&[][..], Vec::as_slice);
            if let Some(status) = status(reference, rows, &targets, target_sources) {
                record(source, proof, reference, status, fields);
            }
        }
    }
    Ok(())
}

fn status(
    reference: &CatalogReference,
    rows: &CatalogRows,
    targets: &TargetIndex,
    sources: &[&SourceObservation],
) -> Option<ExportReferenceStatus> {
    if catalog_reference::actual_kind(&reference.target_type) != Some(reference.target_kind) {
        return Some(ExportReferenceStatus::UnexpectedTargetType);
    }
    let identity = (reference.target_kind, reference.target_guid);
    if targets.ambiguous.contains(&identity) {
        return Some(ExportReferenceStatus::AmbiguousIdentity);
    }
    if targets
        .emitted
        .contains(&(reference.target_kind, reference.target_record_index))
    {
        return None;
    }
    if targets
        .emitted_by_guid
        .get(&identity)
        .is_some_and(|origins| !origins.is_empty())
    {
        return Some(ExportReferenceStatus::AmbiguousIdentity);
    }
    if !reference
        .target_kind
        .records()
        .matches_path(&reference.target_record_path)
    {
        return Some(ExportReferenceStatus::OutsideCatalogSelection);
    }
    if rows
        .filter
        .as_ref()
        .is_some_and(|filter| !filter.matches(&reference.target_record_path))
    {
        return Some(ExportReferenceStatus::ExcludedByFilter);
    }
    if let Some(reason) = targets
        .excluded
        .get(&(reference.target_kind, reference.target_record_index))
    {
        return Some(match reason {
            CatalogRowExclusion::Filter => ExportReferenceStatus::ExcludedByFilter,
            CatalogRowExclusion::DevelopmentLeftover => ExportReferenceStatus::ExcludedByPolicy,
        });
    }
    Some(source_status(reference.target_kind, sources))
}

fn source_status(kind: crate::Kind, sources: &[&SourceObservation]) -> ExportReferenceStatus {
    if sources.iter().any(|source| {
        source.disposition == SourceDisposition::ReadFailed
            && (source.kind == Some(kind)
                || matches!(
                    source.role,
                    ExportSourceRole::Dependency | ExportSourceRole::SelectionCandidate
                ))
    }) {
        return ExportReferenceStatus::TargetReadFailed;
    }
    if sources.iter().any(|source| {
        source.kind == Some(kind) && source.disposition == SourceDisposition::ProjectionFailed
    }) {
        return ExportReferenceStatus::TargetProjectionFailed;
    }
    if sources.iter().any(|source| {
        source.kind == Some(kind)
            && matches!(
                source.disposition,
                SourceDisposition::SkippedBeforeRead | SourceDisposition::SkippedAfterRead
            )
    }) {
        return ExportReferenceStatus::ExcludedByPolicy;
    }
    ExportReferenceStatus::TargetNotEmitted
}

fn record(
    source: &SourceObservation,
    proof: &FieldInspection,
    reference: &CatalogReference,
    status: ExportReferenceStatus,
    fields: &mut Fields,
) {
    fields.references.push(ExportUnresolvedReference {
        record_index: source.record_index,
        kind: source.kind,
        source_role: source.role,
        record_guid: proof.guid.to_string(),
        record_path: proof.record_path.clone(),
        field: reference.field.clone(),
        source_type: reference.source_type.clone(),
        target_guid: Some(reference.target_guid.to_string()),
        target_record_index: Some(reference.target_record_index),
        target_record_path: Some(reference.target_record_path.clone()),
        target_type: Some(reference.target_type.clone()),
        target_location: None,
        status,
        source_pointer: None,
    });
    let blocking = matches!(
        status,
        ExportReferenceStatus::AmbiguousIdentity
            | ExportReferenceStatus::TargetNotEmitted
            | ExportReferenceStatus::UnexpectedTargetType
    );
    fields.diagnostics.push(ExportDiagnostic {
        kind: source.kind,
        stage: ExportDiagnosticStage::SourceField,
        code: if status == ExportReferenceStatus::AmbiguousIdentity {
            ExportDiagnosticCode::AmbiguousIdentity
        } else {
            ExportDiagnosticCode::CatalogTargetUnavailable
        },
        severity: if blocking {
            ExportDiagnosticSeverity::Error
        } else {
            ExportDiagnosticSeverity::Information
        },
        source_role: Some(source.role),
        warning_code: None,
        count: 1,
        count_unit: ExportDiagnosticUnit::Values,
        examples: vec![ExportDiagnosticExample {
            record_index: Some(source.record_index),
            record_guid: Some(proof.guid.to_string()),
            record_path: Some(proof.record_path.clone()),
            field: Some(reference.field.clone()),
            source_type: Some(reference.source_type.clone()),
            target_guid: Some(reference.target_guid.to_string()),
        }],
    });
}

fn ambiguous_rows(
    targets: &TargetIndex,
    canonical: &BTreeMap<u32, FieldInspection>,
    fields: &mut Fields,
) -> Result<(), ExportAccountingError> {
    for (kind, physical) in &targets.ambiguous_sources {
        let proof = canonical
            .get(physical)
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        fields.diagnostics.push(ExportDiagnostic {
            kind: Some(*kind),
            stage: ExportDiagnosticStage::Projection,
            code: ExportDiagnosticCode::AmbiguousIdentity,
            severity: ExportDiagnosticSeverity::Error,
            source_role: Some(ExportSourceRole::Selected),
            warning_code: None,
            count: 1,
            count_unit: ExportDiagnosticUnit::Records,
            examples: vec![ExportDiagnosticExample {
                record_index: Some(*physical),
                record_guid: Some(proof.guid.to_string()),
                record_path: Some(proof.record_path.clone()),
                field: None,
                source_type: None,
                target_guid: Some(proof.guid.to_string()),
            }],
        });
    }
    Ok(())
}
