// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::SourceDisposition;
use crate::{HealthText, Kind};

use super::Evidence;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn catalog_and_variant_evidence_share_the_same_successful_projection() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Tag").field("tagName", TestType::String))
        .record(
            TestRecord::new(
                0x6700_0000_0000_4000_8000_0000_0000_0001,
                "Tag",
                "Tag.Synthetic",
                "synthetic/tag.xml",
            )
            .value("tagName", TestValue::text("Unmanned")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let (legacy, failures) = Evidence::read(&records);
    let (catalog, catalog_failures, ledger) = Evidence::read_catalog(&records, None)?;
    assert!(
        failures.is_empty() && catalog_failures.is_empty(),
        "the canonical Tag reads succeeded"
    );
    assert_eq!(
        catalog.unmanned, legacy.unmanned,
        "both callers use the same typed projection"
    );
    let source = ledger.sources.first().ok_or("dependency missing")?;
    assert_eq!(
        source.kind,
        Some(Kind::Ships),
        "playability evidence belongs to ships"
    );
    assert_eq!(
        source.role,
        ExportSourceRole::Dependency,
        "the Tag is not a selected ship"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::Projected,
        "the actual successful read is captured"
    );
    assert_eq!(
        ledger.field_inspections.len(),
        1,
        "the canonical Tag was inspected once"
    );
    Ok(())
}

#[test]
fn failed_manifest_reads_retain_exact_dependency_identities() -> TestResult {
    let bytes = crate::crafting::ledger::fixtures::broken(
        "CrewManifest",
        "CrewManifest.Synthetic",
        "synthetic/crew.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let (evidence, failures, ledger) = Evidence::read_catalog(&records, None)?;
    let source = ledger.sources.first().ok_or("failed dependency missing")?;
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "the canonical read failed before projection"
    );
    assert_eq!(
        source.record_path.as_ref().map(HealthText::as_str),
        Some("synthetic/crew"),
        "the exact dependency path is retained"
    );
    assert!(
        evidence.unreadable.contains(&source.guid),
        "ship playability retains the unreadable dependency identity"
    );
    assert_eq!(
        failures.len(),
        1,
        "the compatibility failure accompanies the production observation"
    );
    assert!(
        ledger.field_inspections.is_empty(),
        "a failed canonical read has no field proof"
    );
    Ok(())
}

#[test]
fn optional_evidence_helpers_preserve_actual_field_errors() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("CrewManifest").field("bad", TestType::Bool))
        .record(
            TestRecord::new(
                0x6700_0000_0000_4000_8000_0000_0000_0002,
                "CrewManifest",
                "CrewManifest.Synthetic",
                "synthetic/crew.xml",
            )
            .value("bad", TestValue::Raw(vec![2])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let (evidence, failures, ledger) = Evidence::read_catalog(&records, None)?;
    assert!(
        evidence.crewed.is_empty() && failures.is_empty(),
        "an empty successful projection is not a tree-read failure"
    );
    assert_eq!(
        ledger
            .sources
            .first()
            .ok_or("dependency missing")?
            .disposition,
        SourceDisposition::Projected,
        "the successful zero-result branch is recorded explicitly"
    );
    assert!(
        ledger
            .field_inspections
            .iter()
            .flat_map(|inspection| &inspection.diagnostics)
            .any(|diagnostic| diagnostic.code == ExportDiagnosticCode::UnreadableSource),
        "an invalid source field is not silently converted to an absent optional field"
    );
    Ok(())
}
