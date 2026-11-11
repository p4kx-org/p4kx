// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForgeError;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::TestResult;
use crate::build::original_input::test_support;
use crate::export_health::CatalogPublicationMode;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::record_observation::RecordObservation;
use crate::{
    CatalogError, CatalogReadFailure, CraftingWarning, ExportAccountingError, ExportSourceRole,
    Kind,
};

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("AccountingSource")
                .field("value", TestType::UInt32)
                .declared_size(5),
        )
        .record(
            TestRecord::new(
                0x7131,
                "AccountingSource",
                "AccountingSource.One",
                "synthetic/1",
            )
            .value("value", TestValue::UInt32(11)),
        )
        .build()
}

pub(super) fn finish_json(
    failed: bool,
    warnings: &[CraftingWarning],
) -> TestResult<serde_json::Value> {
    let mut output = None;
    test_support::with_input(&bytes(), |forge, input| {
        let mut ledger = super::ledger();
        let mut failures = Vec::new();
        if failed {
            let record = forge.record(0).ok_or("actual failed original Record")?;
            let error = record.tree().err().ok_or("actual strict source failure")?;
            assert!(
                matches!(
                    error,
                    DataForgeError::StructLayout {
                        struct_index: 0,
                        declared: 5,
                        fields: 4
                    }
                ),
                "the original reader independently proves the source failure"
            );
            let admission = RecordObservation::capture(input, &record)?;
            ledger.observe(admission.source(
                Some(Kind::Blueprints),
                ExportSourceRole::Selected,
                SourceDisposition::ReadFailed,
            )?);
            failures.push(CatalogReadFailure::of(&record, &error));
        }
        let mode = if failed || !warnings.is_empty() {
            CatalogPublicationMode::DiagnosticPartial
        } else {
            CatalogPublicationMode::Normal
        };
        let prepared = crate::export_health_compile::prepare_report(
            ledger,
            &super::rows(Kind::Blueprints, 0),
            failures.iter().map(CatalogReadFailure::guid),
            warnings,
            mode,
        )?;
        let report = prepared.finish(input, &failures, None)?;
        output = Some(serde_json::from_slice(&serde_json::to_vec(&report)?)?);
        Ok(())
    })?;
    output.ok_or_else(|| "the actual finished report was not observed".into())
}

#[test]
fn accounting_descriptors_cannot_pass_the_original_report_finish() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let before = input.origins().uses.freeze()?;
        let mut ledger = super::ledger();
        ledger.observe(SourceObservation::accounting(
            Some(Kind::Blueprints),
            record.index(),
            record.guid(),
            Some(crate::HealthText::new(record.record_path()?)),
            ExportSourceRole::Selected,
            SourceDisposition::ReadFailed,
        ));
        let prepared = crate::export_health_compile::prepare_report(
            ledger,
            &super::rows(Kind::Blueprints, 0),
            [],
            &[],
            CatalogPublicationMode::DiagnosticPartial,
        )?;
        assert!(
            matches!(
                prepared.finish(input, &[], None),
                Err(CatalogError::Accounting(
                    ExportAccountingError::UnboundHealthSubject
                ))
            ),
            "matching actual header descriptors never replace a captured original admission"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "rejected accounting capture registers no original header output use"
        );
        Ok(())
    })
}
