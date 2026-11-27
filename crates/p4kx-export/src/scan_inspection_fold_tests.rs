// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::ScanInspectionFold;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original;
use crate::catalog_row::CatalogRows;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::FieldInspections;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::{ExportAccountingError, ExportReferenceStatus, Kind};

#[test]
fn nonadjacent_dependency_proofs_share_one_inspection_and_keep_exact_reference_rows() -> TestResult
{
    test_support::with_input(&database(), |forge, input| {
        let mut baseline = ExportLedger::default();
        let mut folded = ExportLedger::default();
        let mut fold = ScanInspectionFold::start(&mut folded);
        for (physical, kind) in [(0, Kind::Items), (1, Kind::Items), (0, Kind::Ships)] {
            let read = inspect(forge, input, physical, kind)?;
            baseline.merge(read.clone());
            fold.merge(&mut folded, read);
        }

        assert_indices(&folded, &[0, 1]);
        assert_eq!(
            folded.sources.len(),
            3,
            "all original dependency roles stay live"
        );
        let base = resolve(baseline)?;
        let actual = resolve(folded)?;
        assert_eq!(
            actual.diagnostics, base.diagnostics,
            "diagnostics have the same order, samples and counts",
        );
        assert_eq!(
            actual.references, base.references,
            "both exact ordered owner outcomes survive the early fold",
        );
        let published = original::finish_report(
            input,
            &[],
            actual.failed,
            Vec::new(),
            actual.diagnostics,
            actual.references,
            None,
        )?;
        let [item, ship] = published.references.as_slice() else {
            return Err("two retained dependency outcomes".into());
        };
        for reference in [item, ship] {
            assert_eq!(
                (
                    reference.target_record_index().value(),
                    reference.field_text().value()
                ),
                (Some(&2), Some("link")),
                "the physical target and caller field survive",
            );
            assert_eq!(
                reference.status(),
                ExportReferenceStatus::TargetNotEmitted,
                "the final catalog rows determine the reference outcome",
            );
        }
        assert_eq!(
            item.target_record_index().origin().source_uses_refs(),
            ship.target_record_index().origin().source_uses_refs(),
            "the two contexts retain the same actual caller and target selectors",
        );
        Ok(())
    })
}

#[test]
fn conflicting_nonadjacent_inspections_stay_in_original_order_for_final_validation() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let mut ledger = inspect(forge, input, 0, Kind::Items)?;
        let mut fold = ScanInspectionFold::start(&mut ledger);
        fold.merge(&mut ledger, inspect(forge, input, 1, Kind::Items)?);
        let mut conflict = inspect(forge, input, 0, Kind::Ships)?;
        let [proof] = conflict.field_inspections.as_mut_slice() else {
            return Err("one actual conflicting proof".into());
        };
        proof.complete = !proof.complete;
        fold.merge(&mut ledger, conflict);
        fold.merge(&mut ledger, inspect(forge, input, 0, Kind::Ships)?);
        assert_indices(&ledger, &[0, 1, 0, 0]);
        assert!(
            matches!(
                FieldInspections::index(ledger.field_inspections),
                Err(ExportAccountingError::ConflictingFieldInspection)
            ),
            "the original final validator reports the first conflicting shape",
        );
        Ok(())
    })
}

fn assert_indices(ledger: &ExportLedger, expected: &[u32]) {
    let actual: Vec<_> = ledger
        .field_inspections
        .iter()
        .map(|proof| proof.record_index)
        .collect();
    assert_eq!(
        actual, expected,
        "the physical first-appearance order is stable"
    );
}

fn resolve(ledger: ExportLedger) -> TestResult<crate::export_health_fields::Fields> {
    let sources = crate::export_health_records::sources(ledger.sources)?;
    let fields = crate::export_health_fields::resolve(
        ledger.field_inspections,
        &sources,
        Some(&CatalogRows::default()),
        &[],
    )?;
    Ok(fields)
}

fn inspect(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    index: u32,
    kind: Kind,
) -> TestResult<ExportLedger> {
    let record = forge.record(index).ok_or("physical record")?;
    let tree = record.tree()?;
    let admission = RecordObservation::capture(input, &record)?;
    let role = ExportSourceRole::Dependency;
    let mut read = crate::export_field_walk::inspect_observed(
        Some(input),
        &tree,
        &admission,
        Some(kind),
        role,
        None,
    )?;
    read.observe(admission.source(Some(kind), role, SourceDisposition::Projected)?);
    Ok(read)
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owner").field(
            "link",
            TestType::ReferenceTo("SCItemManufacturer".to_owned()),
        ))
        .structure(TestStruct::new("Other").field("value", TestType::UInt32))
        .structure(TestStruct::new("SCItemManufacturer"))
        .record(
            TestRecord::new(1, "Owner", "Owner.One", "owner.xml")
                .value("link", TestValue::Reference(3)),
        )
        .record(
            TestRecord::new(2, "Other", "Other.Two", "other.xml")
                .value("value", TestValue::UInt32(7)),
        )
        .record(TestRecord::new(
            3,
            "SCItemManufacturer",
            "SCItemManufacturer.Target",
            "libs/foundry/records/scitemmanufacturer/target.xml",
        ))
        .build()
}
