// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticStage};

use super::super::error::{
    ArrayShape, Failure, MetadataFamily, ProductionFailure, SourceFault, SourceLevel,
};
use super::super::{TestResult, health};
use super::fixture::Fixture;
use super::{privacy, wire};

#[test]
fn technical_schema_prefixes_classify_only_the_exact_owner_without_exposing_names() -> TestResult {
    let fixture = Fixture::new()?;
    let mut document = fixture.catalog()?;
    let diagnostics = vec![
        wire::diagnostic(&mut document, 1, "LocationCondition_")?,
        wire::diagnostic(&mut document, 0, "MissionPropertyValue_")?,
    ];
    wire::diagnostics(&mut document, diagnostics)?;
    let report = health::read(&document)?;
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
    let diagnostic = wire::diagnostic(&mut document, 0, "DataSetMatchCondition_")?;
    wire::diagnostics(&mut document, vec![diagnostic])?;
    let report = health::read(&document)?;
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
    let mut document = fixture.catalog()?;
    let diagnostic = wire::diagnostic(&mut document, 1, "MissionPropertyValue_")?;
    wire::diagnostics(&mut document, vec![diagnostic])?;
    let report = health::read(&document)?;
    assert_eq!(
        health::unsupported_family(&report, 0),
        MetadataFamily::ExampleUnavailable,
        "bounded examples do not establish the type of an unsampled owner"
    );
    let mut dependency = wire::diagnostic(&mut document, 0, "MissionPropertyValue_")?;
    *dependency
        .get_mut("sourceRole")
        .ok_or("source role absent")? = serde_json::json!("dependency");
    document
        .pointer_mut("/exportHealth/diagnostics")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("diagnostics absent")?
        .push(dependency);
    let report = health::read(&document)?;
    assert_eq!(
        health::unsupported_family(&report, 0),
        MetadataFamily::ExampleUnavailable,
        "a dependency example does not prove a selected-input failure"
    );
    let additional = [
        wire::diagnostic(&mut document, 0, "MissionPropertyValue_")?,
        wire::diagnostic(&mut document, 0, "LocationCondition_")?,
    ];
    document
        .pointer_mut("/exportHealth/diagnostics")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("diagnostics absent")?
        .extend(additional);
    let report = health::read(&document)?;
    assert_eq!(
        health::unsupported_family(&report, 0),
        MetadataFamily::ConflictingExamples,
        "conflicting metadata is never silently reduced to one family"
    );
    Ok(())
}
