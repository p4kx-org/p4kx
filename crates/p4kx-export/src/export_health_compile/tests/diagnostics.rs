// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::export_diagnostic::{
    EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT, ExportDiagnosticCode, ExportSourceRole,
};
use crate::export_health::CatalogPublicationMode;
use crate::export_health_compile;
use crate::export_ledger::{ExportAccountingError, SourceDisposition};
use crate::{CatalogFailure, CraftingWarning, Kind};

use super::TestResult;

#[test]
fn diagnostic_samples_are_bounded_while_failed_identities_are_complete() -> TestResult {
    let mut ledger = super::ledger();
    for id in 1..=21 {
        super::observe(
            &mut ledger,
            super::source(
                Kind::Items,
                id,
                ExportSourceRole::Selected,
                SourceDisposition::ReadFailed,
            ),
        );
    }
    let mut reverse = ledger.clone();
    reverse.sources.reverse();
    let report = export_health_compile::compile(
        ledger,
        &super::rows(Kind::Items, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let reversed = export_health_compile::compile(
        reverse,
        &super::rows(Kind::Items, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    assert_eq!(
        report, reversed,
        "observation completion order cannot change the artifact"
    );
    assert_eq!(
        report.failed_records.len(),
        21,
        "retention identities are never sampled away"
    );
    let diagnostic = report.diagnostics.first().ok_or("source diagnostic")?;
    assert_eq!(
        diagnostic.count, 21,
        "the full diagnostic count is retained"
    );
    assert_eq!(
        diagnostic.examples.len(),
        EXPORT_DIAGNOSTIC_EXAMPLE_LIMIT,
        "only deterministic examples are bounded"
    );
    Ok(())
}

#[test]
fn warnings_keep_their_stable_code_without_copying_raw_detail_values() -> TestResult {
    let warning = CraftingWarning::UnknownModifierRanges {
        count: 3,
        details: vec!["confidential synthetic payload".to_owned()],
    };
    let report = export_health_compile::compile(
        super::ledger(),
        &super::rows(Kind::Blueprints, 0),
        &[],
        &[warning],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let diagnostic = report.diagnostics.first().ok_or("warning diagnostic")?;
    assert_eq!(
        diagnostic.code,
        ExportDiagnosticCode::SourceWarning,
        "source warnings have an explicit semantic category"
    );
    assert_eq!(
        diagnostic.warning_code.as_deref(),
        Some("unknown_modifier_ranges"),
        "the existing warning code is reused"
    );
    assert_eq!(
        diagnostic.count, 3,
        "the producer's actual count is retained"
    );
    assert!(
        !serde_json::to_string(&report)?.contains("confidential synthetic payload"),
        "raw values are excluded from artifact diagnostic messages"
    );
    assert!(
        !report.permits_publication(CatalogPublicationMode::Normal),
        "a source warning blocks normal publication"
    );
    assert!(
        report.permits_publication(CatalogPublicationMode::DiagnosticPartial),
        "only explicit diagnosis permits the incomplete artifact"
    );
    Ok(())
}

#[test]
fn every_compatibility_failure_requires_a_production_explanation() {
    let source = super::source(
        Kind::Blueprints,
        1,
        ExportSourceRole::Selected,
        SourceDisposition::ProjectionFailed,
    );
    let failure = CatalogFailure {
        guid: source.guid,
        record_path: "synthetic/1".to_owned(),
        reason: "private source failure text".to_owned(),
    };
    let result = export_health_compile::compile(
        super::ledger(),
        &super::rows(Kind::Blueprints, 0),
        &[failure],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    );
    assert!(
        matches!(result, Err(ExportAccountingError::UnobservedFailure)),
        "the summary cannot report a failure missing from the artifact ledger"
    );
}

#[test]
fn a_file_only_roundtrip_distinguishes_failure_from_successful_absence() -> TestResult {
    let mut failed = super::ledger();
    super::observe(
        &mut failed,
        super::source(
            Kind::Blueprints,
            1,
            ExportSourceRole::Selected,
            SourceDisposition::ProjectionFailed,
        ),
    );
    let failed = export_health_compile::compile(
        failed,
        &super::rows(Kind::Blueprints, 0),
        &[],
        &[],
        CatalogPublicationMode::DiagnosticPartial,
    )?;
    let empty = export_health_compile::compile(
        super::ledger(),
        &super::rows(Kind::Blueprints, 0),
        &[],
        &[],
        CatalogPublicationMode::Normal,
    )?;
    let failed: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(&failed)?)?;
    let empty: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(&empty)?)?;
    assert_eq!(
        failed.get("writtenRows"),
        empty.get("writtenRows"),
        "both artifacts can have no rows"
    );
    assert_eq!(
        failed.get("status").and_then(serde_json::Value::as_str),
        Some("incomplete"),
        "the file itself identifies a failed projection"
    );
    assert_eq!(
        empty.get("status").and_then(serde_json::Value::as_str),
        Some("complete"),
        "successful absence remains a distinct state"
    );
    assert_ne!(
        failed.get("failedRecords"),
        empty.get("failedRecords"),
        "the affected source identity travels with diagnostic data"
    );
    Ok(())
}
