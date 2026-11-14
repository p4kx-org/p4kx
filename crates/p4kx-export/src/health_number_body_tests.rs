// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    DataForge, DataType, SourceLocation, SourceUseRole, SourceValueState, Value,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::health_example_retention::RetainedExampleBody;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support;
use crate::catalog_number_capture::FloatToken;
use crate::catalog_number_health::NumberCaptures;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportSourceRole};
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, Kind};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const BITS: u32 = 0x7fc0_1234;

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Owner").field("amount", TestType::Single))
        .record(
            TestRecord::new(0xB0D1, "Owner", "Owner.Number", "original/number.xml")
                .value("amount", TestValue::Single(f32::from_bits(BITS))),
        )
        .build()
}

fn inspect(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult<(FieldInspection, SourceObservation)> {
    let record = forge.record(0).ok_or("original numeric record")?;
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual original Record/Tree pair");
    assert_eq!(tree.error_count(), 0, "the source scalar is readable");
    assert_eq!(tree.fields().len(), 1, "one original scalar declaration");
    let field = tree
        .fields()
        .first()
        .ok_or("original numeric declaration")?;
    assert_eq!(field.name(), "amount", "actual original field name");
    assert_eq!(
        field.storage().data_type(),
        DataType::Single,
        "actual original numeric storage"
    );
    assert!(
        matches!(field.value(), Value::Single(value) if value.to_bits() == BITS),
        "exact original nonfinite bits"
    );
    let admission = RecordObservation::capture(input, &record)?;
    let mut ledger = crate::export_field_walk::inspect_observed(
        Some(input),
        &tree,
        &admission,
        Some(Kind::ResourceTypes),
        ExportSourceRole::Selected,
        None,
    )?;
    let inspection = ledger
        .field_inspections
        .pop()
        .ok_or("original numeric inspection")?;
    assert_eq!(
        inspection.number_requirements.len(),
        1,
        "one original token requires coverage"
    );
    let required = inspection
        .number_requirements
        .first()
        .ok_or("original requirement")?;
    assert_eq!(
        required.token,
        FloatToken::Single(BITS),
        "no numeric token or bit change"
    );
    assert_eq!(
        required.original_ordinal, 0,
        "actual original value position"
    );
    let source = admission.source(
        Some(Kind::ResourceTypes),
        ExportSourceRole::Selected,
        SourceDisposition::Projected,
    )?;
    Ok((inspection, source))
}

#[test]
fn uncovered_numeric_example_keeps_same_site_field_and_type_through_one_seal() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let (inspection, source) = inspect(forge, input)?;
        let captures = NumberCaptures::new(None, None);
        let mut coverage = captures.for_source(&source)?;
        let diagnostic =
            crate::catalog_number_diagnostics::uncovered(&inspection, &source, &mut coverage)?
                .ok_or("genuinely uncovered original number")?;
        assert_eq!(
            diagnostic.code,
            ExportDiagnosticCode::UnsupportedValue,
            "unchanged blocking diagnostic"
        );
        assert_eq!(diagnostic.count, 1, "unchanged original-token count");
        let example = diagnostic
            .examples
            .first()
            .ok_or("retained original numeric example")?;
        let retained = RetainedExampleBody::read(example.body()?.clone(), input)?;
        let texts =
            input
                .origins()
                .health_texts()
                .seal(&input.origins().uses, None, &retained.texts())?;
        let cells = retained.finish(&texts, &input.origins().uses)?;
        assert_eq!(
            cells.field.view().value(),
            Some("amount"),
            "exact original field text"
        );
        assert_eq!(
            cells.source_type.view().value(),
            Some("Owner"),
            "exact original owner type"
        );
        assert_eq!(
            cells.target_guid.state(),
            SourceValueState::Absent,
            "no invented reference target"
        );
        check_origins(&cells, input)?;
        assert_eq!(
            texts.into_metadata().texts().len(),
            2,
            "only retained field and type atoms are sealed"
        );
        assert!(
            matches!(
                input
                    .origins()
                    .health_texts()
                    .seal(&input.origins().uses, None, &[]),
                Err(CatalogError::Accounting(
                    ExportAccountingError::HealthCaptureClosed
                ))
            ),
            "the existing report pool seals once"
        );
        Ok(())
    })
}

fn check_origins(
    cells: &super::health_example_cells::ExampleBodyCells,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult {
    let inventory = input.origins().uses.freeze()?;
    for (cell, role, location) in [
        (
            &cells.field,
            SourceUseRole::Conversion,
            SourceLocation::Subresource {
                namespace: "dataforgeValue".to_owned(),
                indices: vec![0, 0],
                field: "$fieldName".to_owned(),
            },
        ),
        (
            &cells.source_type,
            SourceUseRole::Selector,
            SourceLocation::Subresource {
                namespace: "dataforgeRecordHeader".to_owned(),
                indices: vec![0],
                field: "sourceType".to_owned(),
            },
        ),
    ] {
        let origin = cell.view().origin();
        assert!(
            origin.source_set_complete(),
            "the retained text datum has complete original contributions"
        );
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(false),
            "the actual fixture source is plain"
        );
        assert_eq!(
            origin.source_uses_refs().len(),
            1,
            "exact immediate text contributor set"
        );
        let id = origin
            .source_uses_refs()
            .first()
            .ok_or("retained immediate use")?;
        let used = inventory
            .uses()
            .iter()
            .find(|used| used.id() == id)
            .ok_or("same-report original use")?;
        assert_eq!(used.role(), role, "text role does not become float payload");
        assert_eq!(used.location(), &location, "exact original text coordinate");
    }
    no_float_payload(&inventory);
    Ok(())
}

fn no_float_payload(inventory: &p4kx_extraction::SourceInventory) {
    assert!(
        !inventory
            .uses()
            .iter()
            .any(|used| used.role() == SourceUseRole::Payload
                && used.location()
                    == &SourceLocation::Subresource {
                        namespace: "dataforgeValue".to_owned(),
                        indices: vec![0, 0],
                        field: "$value".to_owned(),
                    }),
        "nonfinite diagnostic existence does not relabel the float as field-name payload"
    );
}

#[test]
fn equal_numeric_requirements_reject_foreign_body_inventory_before_retention() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let (inspection, _) = inspect(forge, input)?;
        let original = inspection
            .number_requirements
            .first()
            .ok_or("own original token")?;
        test_support::with_input(&bytes(), |foreign_forge, foreign_input| {
            let (foreign, _) = inspect(foreign_forge, foreign_input)?;
            let foreign = foreign
                .number_requirements
                .first()
                .ok_or("foreign original token")?;
            assert_eq!(
                original, foreign,
                "all numeric storage values and logical coordinates agree"
            );
            let before = input.origins().uses.freeze()?;
            let mut clone = original.clone();
            assert!(
                matches!(
                    clone.merge(foreign.clone()),
                    Err(ExportAccountingError::ConflictingSourceDatum)
                ),
                "equal token metadata cannot admit a foreign original body"
            );
            assert_eq!(
                before,
                input.origins().uses.freeze()?,
                "failed foreign merge retains no contributions"
            );
            Ok(())
        })
    })
}

#[test]
fn genuine_numeric_clone_merges_but_an_accounting_only_body_cannot_replace_it() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let (inspection, _) = inspect(forge, input)?;
        let original = inspection
            .number_requirements
            .first()
            .ok_or("own original numeric token")?;
        let mut clone = original.clone();
        clone.merge(original.clone())?;
        let mut missing = original.clone();
        missing.body = None;
        assert_eq!(
            &missing, original,
            "removing source evidence changes no numeric value"
        );
        let before = input.origins().uses.freeze()?;
        assert!(
            matches!(
                clone.merge(missing),
                Err(ExportAccountingError::UnboundHealthSubject)
            ),
            "an accounting-only requirement cannot replace genuine captured body evidence"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "failed evidence substitution adds no contributions"
        );
        Ok(())
    })
}
