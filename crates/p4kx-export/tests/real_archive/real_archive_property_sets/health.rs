// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    ExportDiagnosticCode, ExportDiagnosticStage, ExportHealth, ExportSourceRole, Kind,
};

use super::OracleResult;
use super::error::{Failure, HealthFault, MetadataFamily, ProductionFailure};

pub(super) fn production_failure(health: &ExportHealth, index: u32) -> ProductionFailure {
    let failures = || {
        health.failed_records.iter().filter(|failure| {
            failure.record_index == index
                && failure.kind == Some(Kind::MissionPropertySets)
                && failure.role == ExportSourceRole::DerivedInput
        })
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

pub(super) fn unsupported_family(health: &ExportHealth, index: u32) -> MetadataFamily {
    let diagnostics = health.diagnostics.iter().filter(|diagnostic| {
        diagnostic.kind == Some(Kind::MissionPropertySets)
            && diagnostic.source_role == Some(ExportSourceRole::DerivedInput)
            && diagnostic.code == ExportDiagnosticCode::UnsupportedValue
            && matches!(
                diagnostic.stage,
                ExportDiagnosticStage::Projection | ExportDiagnosticStage::DerivedProjection
            )
    });
    let mut found = None;
    for diagnostic in diagnostics {
        let examples = diagnostic
            .examples
            .iter()
            .filter(|example| example.record_index == Some(index));
        for example in examples {
            let family = example
                .source_type
                .as_deref()
                .map_or(MetadataFamily::MetadataAbsent, schema_family);
            if found.is_some_and(|previous| previous != family) {
                return MetadataFamily::ConflictingExamples;
            }
            found = Some(family);
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

pub(super) fn read_failed(health: &ExportHealth, index: u32) -> bool {
    health.failed_records.iter().any(|failure| {
        failure.record_index == index
            && failure.kind == Some(Kind::MissionPropertySets)
            && failure.role == ExportSourceRole::DerivedInput
            && failure.stage == ExportDiagnosticStage::SourceRead
    })
}

pub(super) fn ambiguous(health: &ExportHealth, index: u32) -> bool {
    health.failed_records.iter().any(|failure| {
        failure.record_index == index
            && failure.kind == Some(Kind::MissionPropertySets)
            && failure.role == ExportSourceRole::DerivedInput
            && failure.stage == ExportDiagnosticStage::DerivedProjection
            && failure.code == ExportDiagnosticCode::AmbiguousIdentity
    })
}

pub(super) fn reconcile(health: &ExportHealth, sources: usize, unreadable: usize) -> OracleResult {
    let kind = health
        .kind(Kind::MissionPropertySets)
        .ok_or(Failure::HealthReconcile(HealthFault::MissingKind))?;
    if kind.source_records.selected != sources {
        return Err(Failure::HealthReconcile(HealthFault::SelectedCount));
    }
    if kind.source_records.omitted_after_read_failure != unreadable {
        return Err(Failure::HealthReconcile(HealthFault::ReadOmissionCount));
    }
    if kind.source_records.omitted_after_projection_failure != 0 {
        return Err(Failure::HealthReconcile(HealthFault::ProjectionOmission));
    }
    if kind.source_records.projected != sources - unreadable {
        return Err(Failure::HealthReconcile(HealthFault::ProjectedCount));
    }
    if kind.source_records.intentionally_skipped != 0 {
        return Err(Failure::HealthReconcile(HealthFault::IntentionalSkip));
    }
    Ok(())
}
