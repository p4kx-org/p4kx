// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::Kind;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_ledger::SourceDisposition;

use super::{SignatureSlot, SlotProblem};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn an_unknown_slot_from_a_readable_source_is_a_projection_warning() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("RadarSystemGlobalParams").field("bad", TestType::Bool))
        .record(
            TestRecord::new(
                0x6800_0000_0000_4000_8000_0000_0000_0001,
                "RadarSystemGlobalParams",
                "RadarSystemGlobalParams.Synthetic",
                "synthetic/radar.xml",
            )
            .value("bad", TestValue::Raw(vec![2])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let (_, failures, problem, ledger) = SignatureSlot::read_catalog(&records, None)?;
    assert_eq!(
        problem,
        Some(SlotProblem::NoEntry),
        "the readable source lacks the configured slot"
    );
    assert!(
        failures.is_empty(),
        "no canonical read failure was invented"
    );
    let source = ledger.sources.first().ok_or("dependency missing")?;
    assert_eq!(
        source.kind,
        Some(Kind::MineableEntities),
        "the radar dependency belongs to scan-signature users"
    );
    assert_eq!(
        source.role,
        ExportSourceRole::Dependency,
        "the radar parameters are not a selected entity"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::Projected,
        "the source projection completed with its established warning"
    );
    assert!(
        ledger
            .field_inspections
            .iter()
            .flat_map(|inspection| &inspection.diagnostics)
            .any(|diagnostic| diagnostic.code == ExportDiagnosticCode::UnreadableSource),
        "optional slot helpers retain unrelated source-field decoding errors"
    );
    Ok(())
}

#[test]
fn a_failed_radar_read_is_distinct_from_a_missing_entry() -> TestResult {
    let bytes = crate::crafting::ledger::fixtures::broken(
        "RadarSystemGlobalParams",
        "RadarSystemGlobalParams.Synthetic",
        "synthetic/radar.xml",
    );
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let (_, failures, problem, ledger) = SignatureSlot::read_catalog(&records, None)?;
    assert_eq!(
        problem, None,
        "a read failure does not fabricate evidence that the entry is absent"
    );
    assert_eq!(failures.len(), 1, "the canonical failure is retained");
    let source = ledger
        .sources
        .first()
        .ok_or("failed radar dependency missing")?;
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "the canonical reader failed before slot projection"
    );
    assert_eq!(
        source.guid,
        failures
            .first()
            .ok_or("compatibility failure missing")?
            .guid,
        "both representations identify the same failed source"
    );
    assert!(
        ledger.field_inspections.is_empty(),
        "no canonical field proof exists for this failed read"
    );
    Ok(())
}
