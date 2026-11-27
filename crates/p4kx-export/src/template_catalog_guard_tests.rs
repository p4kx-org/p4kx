// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceEvidenceError, Value};

use super::{TemplateFieldCapture, fixture};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, HealthText};

#[test]
fn equal_original_fields_cannot_use_another_records_catalog_context() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual owning Record")?;
        let other = forge.record(1).ok_or("actual different Record")?;
        let tree = record.tree()?;
        let other_tree = other.tree()?;
        fixture::original(&tree)?;
        fixture::original(&other_tree)?;
        assert_eq!(
            tree.field("value"),
            other_tree.field("value"),
            "equal actual source values"
        );
        assert_ne!(
            record.index(),
            other.index(),
            "different actual Record positions"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let own = input.origins().record(record.index());
        let checked = CatalogFields::observed(tree.root(), "", &own, &capture)?;
        assert_eq!(
            checked.boolean("value", &mut Vec::new())?.value(),
            Some(&true),
            "the same original Context remains valid"
        );
        let foreign = input.origins().record(other.index());
        let before = scope.sources().freeze()?;
        let Err(error) = CatalogFields::observed(tree.root(), "", &foreign, &capture) else {
            return Err("another Record Context must be rejected before a valid getter".into());
        };
        assert!(
            matches!(
                error,
                CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
            ),
            "another actual Record remains a typed source rejection"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "rejection adds no source contributions"
        );
        Ok(())
    })
}

#[test]
fn equal_archive_metadata_cannot_replace_the_original_catalog_context() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual owning Record")?;
        let tree = record.tree()?;
        fixture::original(&tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        test_support::with_input(&fixture::bytes(), |other_forge, other_input| {
            let other = other_forge
                .record(0)
                .ok_or("equal genuine foreign Record")?;
            let other_tree = other.tree()?;
            fixture::original(&other_tree)?;
            assert_eq!(
                record.index(),
                other.index(),
                "equal original physical indices"
            );
            assert_eq!(record.guid(), other.guid(), "equal authored GUIDs");
            assert_eq!(
                tree.field("value"),
                other_tree.field("value"),
                "equal original fields"
            );
            let context = other_input.origins().record(other.index());
            let before = scope.sources().freeze()?;
            let Err(error) = CatalogFields::observed(tree.root(), "", &context, &capture) else {
                return Err(
                    "another original Archive Context cannot substitute equal metadata".into(),
                );
            };
            assert!(
                matches!(
                    error,
                    CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
                ),
                "foreign input remains a typed source rejection"
            );
            assert_eq!(
                scope.sources().freeze()?,
                before,
                "foreign input adds no source claim"
            );
            Ok(())
        })
    })
}

#[test]
fn a_cargo_slot_context_cannot_replace_record_path_coordinates() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let tree = record.tree()?;
        fixture::original(&tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let context = input.origins().record(record.index()).cargo_slot(0);
        let before = scope.sources().freeze()?;
        let Err(error) = CatalogFields::observed(tree.root(), "", &context, &capture) else {
            return Err(
                "another coordinate representation cannot enter Record field getters".into(),
            );
        };
        assert!(
            matches!(
                error,
                CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
            ),
            "coordinate mismatch is an explicit source rejection"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "coordinate rejection adds no claims"
        );
        Ok(())
    })
}

#[test]
fn unsupported_catalog_structs_retain_their_actual_type_and_error_receipt() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let tree = record.tree()?;
        fixture::original(&tree)?;
        let Some(Value::Struct(nested)) = tree.field("nested") else {
            return Err("actual original nested Struct".into());
        };
        assert_eq!(
            nested.type_name(),
            "Mystery",
            "actual concrete nested source type"
        );
        assert_eq!(
            nested.field("payload"),
            Some(&Value::String("kept")),
            "actual unknown payload"
        );
        let scope = input.record_scope(&record, &tree)?;
        let original = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&original, input, &admission)?;
        let context = input.origins().record(record.index());
        for (source, path, expected_type) in
            [(nested, "nested", "Mystery"), (tree.root(), "", "Owner")]
        {
            let fields = CatalogFields::observed(source, path, &context, &capture)?;
            assert_unsupported_receipt(&fields, path, expected_type)?;
        }
        Ok(())
    })
}

fn assert_unsupported_receipt(
    fields: &CatalogFields<'_, '_, '_>,
    path: &str,
    expected_type: &str,
) -> TestResult {
    let mut notes = Vec::new();
    let cell = fields.unsupported::<bool>(&mut notes)?;
    assert_eq!(
        serde_json::to_value(cell)?.get("state"),
        Some(&serde_json::json!("unsupported")),
        "the existing required interpretation state is preserved"
    );
    assert_eq!(notes.len(), 1, "one original unsupported Struct note");
    let note = notes.first().ok_or("actual unsupported Struct note")?;
    assert_eq!(
        note.description().field,
        path,
        "the same actual unsupported field position"
    );
    assert_eq!(
        note.description().source_type,
        Some(expected_type),
        "this Struct owns its type"
    );
    let body = note
        .body()
        .ok_or("the actual unsupported guard must retain its original receipt")?;
    let (field, source_type, target) = body.description()?;
    assert_eq!(
        field.as_ref().map(HealthText::as_str),
        Some(path),
        "same actual guard path"
    );
    assert_eq!(
        source_type.as_ref().map(HealthText::as_str),
        Some(expected_type),
        "the receipt captures this Struct type rather than its parent"
    );
    assert_eq!(target, None, "this guard does not read a reference target");
    Ok(())
}

#[test]
fn shared_inventory_cannot_substitute_a_different_actual_input_scope() -> TestResult {
    test_support::with_shared_inventory_inputs(
        &fixture::bytes(),
        |forge, input, other_forge, other_input| {
            let record = forge.record(0).ok_or("actual owning Record")?;
            let other = other_forge
                .record(0)
                .ok_or("same-index foreign original Record")?;
            let tree = record.tree()?;
            let other_tree = other.tree()?;
            fixture::original(&tree)?;
            fixture::original(&other_tree)?;
            assert_eq!(
                record.index(),
                other.index(),
                "the actual Record positions coincide"
            );
            assert_eq!(record.guid(), other.guid(), "the actual GUIDs coincide");
            assert_eq!(
                tree.field("value"),
                other_tree.field("value"),
                "original fields coincide"
            );
            let scope = input.record_scope(&record, &tree)?;
            let fields = OriginalFlowFields::read(&scope, None)?;
            let admission = RecordObservation::capture(input, &record)?;
            let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
            let context = other_input.origins().record(other.index());
            assert!(
                scope.sources().is_same_inventory(context.sources()),
                "the countercontrol deliberately keeps the same actual inventory"
            );
            assert!(
                !std::ptr::eq(scope.dataforge(), context.dataforge()),
                "the actual admitted source scopes differ"
            );
            let before = scope.sources().freeze()?;
            let Err(error) = CatalogFields::observed(tree.root(), "", &context, &capture) else {
                return Err(
                    "same inventory and Record position cannot replace the real input scope".into(),
                );
            };
            assert!(
                matches!(
                    error,
                    CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
                ),
                "another actual source scope remains a typed source rejection"
            );
            assert_eq!(
                scope.sources().freeze()?,
                before,
                "rejection adds no foreign contributions"
            );
            Ok(())
        },
    )
}

#[test]
fn equal_source_handle_storage_cannot_replace_the_exact_admitted_scope_borrow() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        let tree = record.tree()?;
        fixture::original(&tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let cloned = scope.dataforge().clone();
        assert_eq!(
            cloned.source_ref(),
            scope.dataforge().source_ref(),
            "the cloned existing handle has the same descriptive source reference"
        );
        assert!(
            !std::ptr::eq(&raw const cloned, scope.dataforge()),
            "the supplied borrow is another object"
        );
        let context = crate::catalog_read::CatalogRecordContext::new(
            scope.sources(),
            &cloned,
            record.index(),
        );
        assert!(
            scope.sources().is_same_inventory(context.sources()),
            "the same actual inventory is retained"
        );
        let before = scope.sources().freeze()?;
        let Err(error) = CatalogFields::observed(tree.root(), "", &context, &capture) else {
            return Err(
                "equal scope storage cannot replace the original bound scope borrow".into(),
            );
        };
        assert!(
            matches!(
                error,
                CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
            ),
            "the exact original binding remains required"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "the adversarial context creates no new claim"
        );
        Ok(())
    })
}
