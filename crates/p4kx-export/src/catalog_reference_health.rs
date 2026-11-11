// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::catalog_read::original::ExampleBodyReceipt;
use crate::catalog_reference;
use crate::catalog_reference::{CatalogReference, CatalogReferenceTarget};
use crate::catalog_row::{CatalogRowExclusion, CatalogRows};
use crate::catalog_target_index::TargetIndex;
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::export_field_inspection::FieldInspection;
use crate::export_health_fields::Fields;
use crate::export_ledger::{ExportAccountingError, SourceDisposition, SourceObservation};
use crate::export_reference_issue::ExportReferenceStatus;
use crate::health_diagnostic_observation::{
    DiagnosticObservation as ExportDiagnostic, ExampleObservation,
};

/// Resolves captured source links using final emitted rows and exact physical source outcomes.
pub(crate) fn resolve(
    canonical: &BTreeMap<u32, FieldInspection>,
    sources: &[SourceObservation],
    rows: Option<&CatalogRows>,
    targets: Option<&TargetIndex>,
    fields: &mut Fields,
) -> Result<(), ExportAccountingError> {
    let (Some(rows), Some(targets)) = (rows, targets) else {
        if canonical
            .values()
            .any(|proof| !proof.catalog_references.is_empty())
        {
            return Err(ExportAccountingError::MissingCatalogRows);
        }
        return Ok(());
    };
    ambiguous_rows(targets, canonical, fields)?;
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
            let target = reference.target()?;
            let target_sources = by_target.get(&target.index).map_or(&[][..], Vec::as_slice);
            if let Some(status) = status(&target, rows, targets, target_sources) {
                record(source, reference, status, fields)?;
            }
        }
    }
    Ok(())
}

fn status(
    target: &CatalogReferenceTarget<'_>,
    rows: &CatalogRows,
    targets: &TargetIndex,
    sources: &[&SourceObservation],
) -> Option<ExportReferenceStatus> {
    if catalog_reference::actual_kind(target.source_type) != Some(target.kind) {
        return Some(ExportReferenceStatus::UnexpectedTargetType);
    }
    let identity = (target.kind, target.guid);
    if targets.ambiguous.contains(&identity) {
        return Some(ExportReferenceStatus::AmbiguousIdentity);
    }
    if targets.emitted.contains(&(target.kind, target.index)) {
        return None;
    }
    if targets
        .emitted_by_guid
        .get(&identity)
        .is_some_and(|origins| !origins.is_empty())
    {
        return Some(ExportReferenceStatus::AmbiguousIdentity);
    }
    if !target.kind.records().matches_path(target.path) {
        return Some(ExportReferenceStatus::OutsideCatalogSelection);
    }
    if target.kind != crate::Kind::Tags
        && rows
            .filter
            .as_ref()
            .is_some_and(|filter| !filter.matches(target.path))
    {
        return Some(ExportReferenceStatus::ExcludedByFilter);
    }
    if let Some(reason) = targets.excluded.get(&(target.kind, target.index)) {
        return Some(match reason {
            CatalogRowExclusion::Filter => ExportReferenceStatus::ExcludedByFilter,
            CatalogRowExclusion::DevelopmentLeftover => ExportReferenceStatus::ExcludedByPolicy,
        });
    }
    Some(source_status(target.kind, sources))
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
    reference: &CatalogReference,
    status: ExportReferenceStatus,
    fields: &mut Fields,
) -> Result<(), ExportAccountingError> {
    let mut original = reference.original.clone().resolved(status);
    original.merge_owner(source.subject())?;
    original.contextualize(source.kind, source.role);
    let data = original.data();
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
        examples: vec![ExampleObservation::captured(
            source.subject().clone(),
            Some(data.field.as_str()),
            Some(data.source_type.as_str()),
            data.target_guid,
            ExampleBodyReceipt::reference(&original)?,
        )?],
    });
    fields.references.push(original);
    Ok(())
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
            examples: vec![ExampleObservation::captured(
                proof.subject.clone(),
                None,
                None,
                Some(proof.guid),
                ExampleBodyReceipt::guid_header(proof.subject.clone()),
            )?],
        });
    }
    Ok(())
}
