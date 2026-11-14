// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod cells;
mod controls;
mod document;

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage};
use serde_json::Value;

use super::OracleResult;
use super::error::{Failure, HealthFault, MetadataFamily, ProductionFailure};

/// One producer document's validated wire facts, without original-source authority.
pub(super) struct Health<'document> {
    _document: &'document Value,
    records: Vec<FailedRecord>,
    diagnostics: Vec<Diagnostic>,
    counts: Counts,
}

struct FailedRecord {
    index: u32,
    relevant: bool,
    stage: ExportDiagnosticStage,
    code: ExportDiagnosticCode,
}

struct Diagnostic {
    relevant: bool,
    examples: Vec<Example>,
}

struct Example {
    index: Option<u32>,
    family: MetadataFamily,
}

struct Counts {
    selected: usize,
    read_omissions: usize,
    projection_omissions: usize,
    projected: usize,
    skipped: usize,
}

pub(super) fn read(document: &Value) -> OracleResult<Health<'_>> {
    document::read(document)
}

pub(super) fn production_failure(health: &Health<'_>, index: u32) -> ProductionFailure {
    let failures = || {
        health
            .records
            .iter()
            .filter(|failure| failure.relevant && failure.index == index)
    };
    failures()
        .find(|failure| {
            matches!(
                failure.stage,
                ExportDiagnosticStage::Projection | ExportDiagnosticStage::DerivedProjection
            )
        })
        .or_else(|| failures().next())
        .map_or(ProductionFailure::NoneReported, |failure| {
            ProductionFailure::Reported {
                stage: failure.stage,
                code: failure.code,
                family: if failure.code == ExportDiagnosticCode::UnsupportedValue {
                    unsupported_family(health, index)
                } else {
                    MetadataFamily::NotApplicable
                },
            }
        })
}

pub(super) fn unsupported_family(health: &Health<'_>, index: u32) -> MetadataFamily {
    let mut found = None;
    let diagnostics = health
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.relevant);
    for diagnostic in diagnostics {
        let examples = diagnostic
            .examples
            .iter()
            .filter(|example| example.index == Some(index));
        for example in examples {
            if found.is_some_and(|previous| previous != example.family) {
                return MetadataFamily::ConflictingExamples;
            }
            found = Some(example.family);
        }
    }
    found.unwrap_or(MetadataFamily::ExampleUnavailable)
}

fn schema_family(name: &str) -> MetadataFamily {
    if name.starts_with("MissionPropertyValue") {
        MetadataFamily::PropertyValue
    } else if name.starts_with("LocationCondition_") {
        MetadataFamily::LocationCondition
    } else if name.starts_with("DataSetMatchCondition_") {
        MetadataFamily::DataSetCondition
    } else {
        MetadataFamily::Other
    }
}

pub(super) fn read_failed(health: &Health<'_>, index: u32) -> bool {
    health.records.iter().any(|failure| {
        failure.relevant
            && failure.index == index
            && failure.stage == ExportDiagnosticStage::SourceRead
    })
}

pub(super) fn ambiguous(health: &Health<'_>, index: u32) -> bool {
    health.records.iter().any(|failure| {
        failure.relevant
            && failure.index == index
            && failure.stage == ExportDiagnosticStage::DerivedProjection
            && failure.code == ExportDiagnosticCode::AmbiguousIdentity
    })
}

pub(super) fn reconcile(health: &Health<'_>, sources: usize, unreadable: usize) -> OracleResult {
    let counts = &health.counts;
    if counts.selected != sources {
        return Err(Failure::HealthReconcile(HealthFault::SelectedCount));
    }
    if counts.read_omissions != unreadable {
        return Err(Failure::HealthReconcile(HealthFault::ReadOmissionCount));
    }
    if counts.projection_omissions != 0 {
        return Err(Failure::HealthReconcile(HealthFault::ProjectionOmission));
    }
    let projected = sources
        .checked_sub(unreadable)
        .ok_or(Failure::HealthReconcile(HealthFault::ProjectedCount))?;
    if counts.projected != projected {
        return Err(Failure::HealthReconcile(HealthFault::ProjectedCount));
    }
    if counts.skipped != 0 {
        return Err(Failure::HealthReconcile(HealthFault::IntentionalSkip));
    }
    Ok(())
}
