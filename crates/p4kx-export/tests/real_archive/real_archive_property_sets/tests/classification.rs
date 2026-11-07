// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{
    ExportDiagnostic, ExportDiagnosticCode, ExportDiagnosticExample, ExportDiagnosticSeverity,
    ExportDiagnosticStage, ExportDiagnosticUnit, ExportSourceRole, Kind,
};

use super::super::error::{
    ArrayShape, Failure, MetadataFamily, ProductionFailure, SourceFault, SourceLevel,
};
use super::super::{TestResult, health};
use super::fixture::Fixture;
use super::privacy;

fn diagnostic(index: u32, prefix: &str) -> ExportDiagnostic {
    ExportDiagnostic {
        kind: Some(Kind::MissionPropertySets),
        stage: ExportDiagnosticStage::DerivedProjection,
        code: ExportDiagnosticCode::UnsupportedValue,
        severity: ExportDiagnosticSeverity::Error,
        source_role: Some(ExportSourceRole::DerivedInput),
        warning_code: Some(privacy::SENTINEL.to_owned()),
        count: 1,
        count_unit: ExportDiagnosticUnit::Records,
        examples: vec![ExportDiagnosticExample {
            record_index: Some(index),
            record_guid: Some(privacy::SENTINEL.to_owned()),
            record_path: Some(privacy::SENTINEL.to_owned()),
            field: Some(privacy::SENTINEL.to_owned()),
            source_type: Some(format!("{prefix}{}", privacy::SENTINEL)),
            target_guid: Some(privacy::SENTINEL.to_owned()),
        }],
    }
}

#[test]
fn technical_schema_prefixes_classify_only_the_exact_owner_without_exposing_names() -> TestResult {
    let fixture = Fixture::new()?;
    let (_, mut report) = fixture.catalog()?;
    report.diagnostics = vec![
        diagnostic(1, "LocationCondition_"),
        diagnostic(0, "MissionPropertyValue_"),
    ];
    let family = health::unsupported_family(&report, 0);
    assert_eq!(
        family,
        MetadataFamily::PropertyValue,
        "another physical owner's schema example cannot classify this source"
    );
    privacy::assert_sanitized(Failure::SourceCompare(SourceFault::MissingAuthoredArray {
        level: SourceLevel::Contract,
        shape: ArrayShape::Populated,
        production: ProductionFailure::Reported {
            stage: ExportDiagnosticStage::DerivedProjection,
            code: ExportDiagnosticCode::UnsupportedValue,
            family,
        },
    }));
    assert_eq!(
        health::unsupported_family(&report, 1),
        MetadataFamily::LocationCondition,
        "location conditions remain a separate static schema family"
    );
    report.diagnostics = vec![diagnostic(0, "DataSetMatchCondition_")];
    assert_eq!(
        health::unsupported_family(&report, 0),
        MetadataFamily::DataSetCondition,
        "the dataset-condition prefix follows the actual spawn reader's declared family"
    );
    Ok(())
}

#[test]
fn absent_sampled_examples_and_conflicting_families_remain_explicitly_unknown() -> TestResult {
    let fixture = Fixture::new()?;
    let (_, mut report) = fixture.catalog()?;
    report.diagnostics = vec![diagnostic(1, "MissionPropertyValue_")];
    assert_eq!(
        health::unsupported_family(&report, 0),
        MetadataFamily::ExampleUnavailable,
        "bounded examples do not establish the type of an unsampled owner"
    );
    let mut dependency = diagnostic(0, "MissionPropertyValue_");
    dependency.source_role = Some(ExportSourceRole::Dependency);
    report.diagnostics.push(dependency);
    assert_eq!(
        health::unsupported_family(&report, 0),
        MetadataFamily::ExampleUnavailable,
        "a dependency example does not prove a selected-input failure"
    );
    report.diagnostics.extend([
        diagnostic(0, "MissionPropertyValue_"),
        diagnostic(0, "LocationCondition_"),
    ]);
    assert_eq!(
        health::unsupported_family(&report, 0),
        MetadataFamily::ConflictingExamples,
        "conflicting metadata is never silently reduced to one family"
    );
    Ok(())
}
