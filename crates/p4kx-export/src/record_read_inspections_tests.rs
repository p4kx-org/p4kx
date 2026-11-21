// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{KindRead, read_kind};
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::{FieldInspection, WholeInspectionFold};
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind};

#[test]
fn mission_definition_and_dependency_keep_both_reads_with_one_whole_inspection() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let mut ledger = dependency_read(forge, input)?;
        let selected = selected_read(forge, input, &mut ledger.field_inspections)?;
        assert_eq!(selected.items.len(), 1, "the selected row is produced");
        assert!(
            selected.ledger.field_inspections.is_empty(),
            "the existing physical proof absorbed the duplicate"
        );
        ledger.merge(selected.ledger);

        let reads: Vec<_> = ledger
            .sources
            .iter()
            .map(|source| {
                (
                    source.record_index,
                    source.kind,
                    source.role,
                    source.disposition,
                )
            })
            .collect();
        assert_eq!(
            reads,
            [
                (
                    0,
                    Some(Kind::Contracts),
                    ExportSourceRole::Dependency,
                    SourceDisposition::Projected
                ),
                (
                    0,
                    Some(Kind::ContractTemplates),
                    ExportSourceRole::Selected,
                    SourceDisposition::Projected
                ),
            ],
            "both successful reads retain their original accounting"
        );
        let [inspection] = ledger.field_inspections.as_slice() else {
            return Err("one whole inspection for the actual source Record".into());
        };
        assert_eq!(inspection.record_index, 0, "the actual source");
        assert_eq!(inspection.kind, None, "canonical whole-Record context");
        Ok(())
    })
}

#[test]
fn mission_definition_rejects_conflicting_earlier_whole_inspection() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let mut ledger = dependency_read(forge, input)?;
        let earlier = ledger
            .field_inspections
            .first_mut()
            .ok_or("dependency inspection")?;
        earlier.complete = !earlier.complete;
        assert!(
            matches!(
                selected_read(forge, input, &mut ledger.field_inspections),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingFieldInspection
                ))
            ),
            "a conflicting shape cannot authenticate a later read"
        );
        Ok(())
    })
}

#[test]
fn selected_definition_without_dependency_retains_its_own_whole_inspection() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let selected = selected_read(forge, input, &mut [])?;
        let [inspection] = selected.ledger.field_inspections.as_slice() else {
            return Err("a fresh whole-Record inspection".into());
        };
        assert_eq!(inspection.record_index, 0, "the actual selected Record");
        assert_eq!(
            inspection.kind,
            Some(Kind::ContractTemplates),
            "selected context"
        );
        Ok(())
    })
}

fn dependency_read(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult<ExportLedger> {
    let record = forge.record(0).ok_or("template Record")?;
    let tree = record.tree()?;
    let admission = RecordObservation::capture(input, &record)?;
    let kind = Some(Kind::Contracts);
    let role = ExportSourceRole::Dependency;
    let mut ledger = crate::export_field_walk::inspect_observed(
        Some(input),
        &tree,
        &admission,
        kind,
        role,
        None,
    )?;
    ledger.observe(admission.source(kind, role, SourceDisposition::Projected)?);
    Ok(ledger)
}

fn selected_read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, '_>,
    earlier: &mut [FieldInspection],
) -> Result<KindRead<'source, CatalogGuid>, CatalogError> {
    let mut inspections = WholeInspectionFold::new(earlier);
    read_kind(
        forge,
        input,
        Kind::ContractTemplates,
        (None, None, Some(&mut inspections)),
        |record, _| Ok(CatalogGuid(record.guid())),
        |guid| *guid,
    )
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("ContractTemplate").field("value", TestType::Int32))
        .record(
            TestRecord::new(
                1,
                "ContractTemplate",
                "ContractTemplate.Selected",
                "libs/foundry/records/contracts/contracttemplates/selected.xml",
            )
            .value("value", TestValue::Int32(7)),
        )
        .build()
}
