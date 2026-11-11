// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    InputScope, SourceEvidenceError, SourceInventory, SourceLocation, SourceUseRole, SourceValue,
    Value,
};

use super::TemplateFieldCapture;
use crate::CatalogError;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::source_fields::{FieldChecks, unknown_struct};
use crate::template_issues::{TemplateDescription, TemplateError};

fn test_error(error: ReadError) -> Box<dyn Error> {
    error
}

#[path = "template_field_capture_fixture.rs"]
mod fixture;

#[path = "template_field_capture_guard_checks.rs"]
mod guard_checks;

#[test]
fn actual_guards_keep_domain_errors_and_defer_all_payload_claims() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original guard Record")?;
        let tree = record.tree()?;
        fixture::original(&tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let before = scope.sources().freeze()?;
        let checked =
            FieldChecks::for_context(tree.root(), "", Some(&capture)).map_err(test_error)?;
        assert_eq!(
            checked.boolean("value").map_err(test_error)?,
            Some(true),
            "actual valid guard is unchanged"
        );
        guard_checks::malformed(&checked)?;
        guard_checks::unreadable(&checked)?;
        let after = scope.sources().freeze()?;
        assert_eq!(
            after, before,
            "unretained guard recipes create no source uses"
        );
        assert_deferred_payloads(&before, &after);
        retain_original_payload(&fields)?;
        Ok(())
    })
}

#[test]
fn actual_array_element_and_unknown_type_failures_keep_their_receipts() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original guard Record")?;
        let tree = record.tree()?;
        fixture::original(&tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let checked =
            FieldChecks::for_context(tree.root(), "", Some(&capture)).map_err(test_error)?;
        guard_checks::array_element(&checked)?;
        let Some(Value::Struct(nested)) = tree.field("nested") else {
            return Err("actual original unknown Struct".into());
        };
        assert_eq!(
            nested.type_name(),
            "Mystery",
            "the original unsupported type was actually read"
        );
        assert_eq!(
            nested.field("payload"),
            Some(&Value::String("kept")),
            "original unknown payload control"
        );
        let unknown = unknown_struct(nested, "property value", Some(&capture));
        let unknown = unknown
            .downcast_ref::<TemplateError>()
            .ok_or("typed actual unknown error")?;
        assert!(
            matches!(
                unknown.description(),
                TemplateDescription::UnknownStruct {
                    field: "property value",
                    name: "Mystery"
                }
            ),
            "required unsupported meaning keeps the original error classification"
        );
        assert!(
            unknown.body().is_some(),
            "unknown type uses its concrete-type receipt"
        );
        Ok(())
    })
}

#[test]
fn equal_foreign_parent_is_a_fatal_source_error_even_for_a_valid_guard() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual owning Record")?;
        let other = forge.record(1).ok_or("actual different original Record")?;
        let tree = record.tree()?;
        let other_tree = other.tree()?;
        fixture::original(&tree)?;
        fixture::original(&other_tree)?;
        assert_eq!(
            tree.field("value"),
            other_tree.field("value"),
            "equal actual payload is not admission"
        );
        assert_ne!(
            record.index(),
            other.index(),
            "the genuine physical owners differ"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let before = scope.sources().freeze()?;
        let Err(error) = FieldChecks::for_context(other_tree.root(), "", Some(&capture)) else {
            return Err("foreign parent cannot enter the guard receiver".into());
        };
        assert!(
            matches!(
                error.downcast_ref::<CatalogError>(),
                Some(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "source rejection stays a top-level CatalogError"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "foreign input creates no new original claim"
        );
        Ok(())
    })
}

#[test]
fn actual_catalog_getters_keep_the_same_guard_state_and_original_body() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original getter Record")?;
        let tree = record.tree()?;
        fixture::original(&tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let original = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&original, input, &admission)?;
        let context = input.origins().record(record.index());
        let fields =
            crate::catalog_read::CatalogFields::observed(tree.root(), "", &context, &capture)?;
        let mut notes = Vec::new();
        let invalid = fields.text("value", &mut notes)?;
        assert_eq!(
            serde_json::to_value(invalid)?.get("state"),
            Some(&serde_json::json!("invalid")),
            "the same real Bool still produces the established invalid String cell"
        );
        let unreadable = fields.boolean("bad", &mut notes)?;
        assert_eq!(
            serde_json::to_value(unreadable)?.get("state"),
            Some(&serde_json::json!("unreadable")),
            "the original malformed Bool remains unreadable"
        );
        guard_checks::getter_notes(&notes)?;
        fields.check_observed(&["value", "bad", "items"], &mut notes)?;
        let unsupported = notes.last().ok_or("actual extra nested declaration")?;
        assert_eq!(
            unsupported.description().field,
            "nested",
            "actual unsupported declaration slot"
        );
        assert!(
            unsupported.body().is_some(),
            "the actual declaration also owns its body"
        );
        assert!(
            scope.sources().freeze()?.uses().iter().all(
                |item| !matches!(item.location(), SourceLocation::Subresource {
                namespace, ..
            } if namespace == "dataforgeValue" && item.role() == SourceUseRole::Payload)
            ),
            "these unretained body recipes add no early original-value payload claims"
        );
        Ok(())
    })
}

#[test]
fn actual_catalog_guard_rejects_a_foreign_original_before_a_valid_getter() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual owning getter Record")?;
        let other = forge.record(1).ok_or("actual foreign getter Record")?;
        let tree = record.tree()?;
        let other_tree = other.tree()?;
        fixture::original(&tree)?;
        fixture::original(&other_tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let original = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&original, input, &admission)?;
        let context = input.origins().record(record.index());
        let Err(error) =
            crate::catalog_read::CatalogFields::observed(other_tree.root(), "", &context, &capture)
        else {
            return Err("equal foreign data cannot enter a Catalog getter receiver".into());
        };
        assert!(
            matches!(
                error,
                CatalogError::SourceEvidence(SourceEvidenceError::ForeignEvidence)
            ),
            "the original source rejection remains a fatal CatalogError"
        );
        Ok(())
    })
}

fn assert_deferred_payloads(before: &SourceInventory, after: &SourceInventory) {
    assert!(before.uses().iter().any(|item|
        item.role() == SourceUseRole::Payload
            && item.scope() == InputScope::ArchiveDirectoryMetadata
            && matches!(item.location(), SourceLocation::Metadata { field } if field == "path")),
        "the actual Archive admission already owns its metadata path payload");
    for inventory in [before, after] {
        assert!(
            inventory
                .uses()
                .iter()
                .all(|item| item.scope() != InputScope::Payload
                    || item.role() != SourceUseRole::Payload),
            "unretained original guard recipes claim no DataForge payload authority"
        );
    }
}

fn retain_original_payload(fields: &OriginalFlowFields<'_, '_, '_>) -> TestResult {
    let first = fields
        .root()
        .fields()
        .first()
        .ok_or("actual first declaration")?;
    assert_eq!(
        first.name(),
        "value",
        "the genuine first original declaration"
    );
    let fact = fields.fact(first.value())?;
    assert_eq!(
        fact.ordinal(),
        0,
        "the independent first field is original occurrence zero"
    );
    let Value::Bool(value) = fact.value() else {
        return Err("actual original Bool".into());
    };
    let cell = fact
        .bind(
            OriginalValuePart::Value,
            SourceValue::Present(*value),
            SourceUseRole::Payload,
            true,
        )?
        .into_cell(fields.sources());
    assert_eq!(
        cell.value(),
        Some(value),
        "the positive control retains the actual original datum"
    );
    assert_eq!(
        cell.origin().source_uses_refs().len(),
        1,
        "exactly one actual payload contributor"
    );
    let inventory = fields.sources().freeze()?;
    let id = cell
        .origin()
        .source_uses_refs()
        .first()
        .ok_or("actual retained contributor")?;
    guard_checks::retained_payload(&inventory, id, fields.record_index())?;
    Ok(())
}

#[path = "template_catalog_guard_tests.rs"]
mod catalog_guards;
