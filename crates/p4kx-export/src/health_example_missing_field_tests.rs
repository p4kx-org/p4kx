// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, SourceEvidenceError, SourceLocation, SourceUseRole, SourceUses, SourcedValue,
};

use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::health_example_body::{ExampleBody, ExampleBodyReceipt};
use crate::catalog_read::original::{ExampleBodyCapture, OriginalFlowFields};
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldCapture;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError, HealthText};

#[path = "health_example_missing_field_fixture.rs"]
mod fixture;

#[test]
fn a_required_missing_declaration_keeps_its_exact_deferred_survey_and_concrete_type() -> TestResult
{
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the canonical read belongs to the actual physical owner"
        );
        let parent = fixture::original(&tree, "nested")?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let fact = fields
            .owner_of(parent)?
            .ok_or("actual nested Struct occurrence")?;
        assert_eq!(
            fact.ordinal(),
            1,
            "flag precedes the nested occurrence in the original walk"
        );
        assert_eq!(
            fact.parent_ordinal(),
            None,
            "the nested Struct is an actual root field"
        );
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let before = scope.sources().freeze()?;
        let nested = capture.missing_field(parent, "start")?;
        let root = capture.missing_field(tree.root(), "requiredRoot")?;
        nested.check_description(Some("nested.start"), Some("Params"), None)?;
        root.check_description(Some("requiredRoot"), Some("Owner"), None)?;
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "unretained absence recipes add no source uses"
        );
        exact_nested(nested, scope.sources())?;
        exact_root(root, scope.sources())
    })
}

#[test]
fn a_second_scope_for_the_same_tree_cannot_supply_a_missing_declaration_receipt() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        let parent = fixture::original(&tree, "nested")?;
        let scope = input.record_scope(&record, &tree)?;
        let other = input.record_scope(&record, &tree)?;
        assert!(
            !std::ptr::eq(&raw const scope, &raw const other),
            "the two scopes have separate actual identities"
        );
        let fields = OriginalFlowFields::read(&scope, None)?;
        let other_fields = OriginalFlowFields::read(&other, None)?;
        let own = fields.missing_field(parent, "start")?;
        let foreign = other_fields.missing_field(parent, "start")?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = ExampleBodyCapture::new(input, &admission, &scope)?;
        let before = scope.sources().freeze()?;
        assert!(
            matches!(
                capture.missing_field(&foreign),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "equal Record and actual tree descriptors do not replace the receiver's borrowed Scope"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "rejection materializes no source contribution"
        );
        capture.missing_field(&own)?.check_description(
            Some("nested.start"),
            Some("Params"),
            None,
        )?;
        Ok(())
    })
}

#[test]
fn missing_declarations_reject_equal_second_read_and_foreign_record_parents() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual first original owner")?;
        let other_record = forge.record(1).ok_or("actual second original owner")?;
        assert_eq!(
            record.guid(),
            other_record.guid(),
            "the actual nominal GUIDs intentionally coincide"
        );
        assert_ne!(
            record.index(),
            other_record.index(),
            "the real physical owners differ"
        );
        let tree = record.tree()?;
        let second = record.tree()?;
        let other_tree = other_record.tree()?;
        let parent = fixture::original(&tree, "nested")?;
        let second_parent = fixture::original(&second, "nested")?;
        let other_parent = fixture::original(&other_tree, "nested")?;
        assert_eq!(
            parent, second_parent,
            "a separate real read has identical visible declarations and values"
        );
        assert_eq!(
            parent, other_parent,
            "a foreign original has identical visible declarations and values"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let before = scope.sources().freeze()?;
        for foreign in [second_parent, other_parent] {
            assert!(
                matches!(
                    capture.missing_field(foreign, "start"),
                    Err(CatalogError::SourceEvidence(
                        SourceEvidenceError::ForeignEvidence
                    ))
                ),
                "actual pointer membership is required before creating any absence fact"
            );
        }
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "foreign parents add no original source use"
        );
        capture.missing_field(parent, "start")?.check_description(
            Some("nested.start"),
            Some("Params"),
            None,
        )?;
        Ok(())
    })
}

#[test]
fn declared_values_and_declared_nulls_are_not_absent_and_cancel_remains_fatal() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        let parent = fixture::original(&tree, "nested")?;
        let scope = input.record_scope(&record, &tree)?;
        let cancel = CancelToken::new();
        let fields = OriginalFlowFields::read(&scope, Some(&cancel))?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let before = scope.sources().freeze()?;
        for declared in ["end", "link"] {
            assert!(
                matches!(
                    capture.missing_field(parent, declared),
                    Err(CatalogError::Accounting(
                        ExportAccountingError::ConflictingSourceDatum
                    ))
                ),
                "present and Null original declarations cannot mint a negative field fact"
            );
        }
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "rejected negative facts retain no contributions"
        );
        cancel.cancel();
        assert!(
            matches!(
                capture.missing_field(parent, "start"),
                Err(CatalogError::Cancelled)
            ),
            "cancellation is not a successful absent-field interpretation"
        );
        Ok(())
    })
}

fn exact_nested(body: ExampleBodyReceipt, sources: &SourceUses) -> TestResult {
    exact_body(
        body,
        sources,
        "nested.start",
        "Params",
        vec![
            coordinate(
                "dataforgeValue",
                vec![0, 1],
                "$fieldName",
                SourceUseRole::Conversion,
            ),
            coordinate(
                "dataforgeValue",
                vec![0, 1],
                "$fields",
                SourceUseRole::Selector,
            ),
        ],
        coordinate(
            "dataforgeValue",
            vec![0, 1],
            "$type",
            SourceUseRole::Selector,
        ),
    )
}

fn exact_root(body: ExampleBodyReceipt, sources: &SourceUses) -> TestResult {
    exact_body(
        body,
        sources,
        "requiredRoot",
        "Owner",
        vec![coordinate(
            "dataforgeRecordDeclarations",
            vec![0],
            "$fields",
            SourceUseRole::Selector,
        )],
        coordinate(
            "dataforgeRecordHeader",
            vec![0],
            "sourceType",
            SourceUseRole::Selector,
        ),
    )
}

type Use = (SourceLocation, SourceUseRole);

fn coordinate(namespace: &str, indices: Vec<u64>, field: &str, role: SourceUseRole) -> Use {
    (
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices,
            field: field.to_owned(),
        },
        role,
    )
}

fn exact_body(
    body: ExampleBodyReceipt,
    sources: &SourceUses,
    path: &str,
    owner: &str,
    field_inputs: Vec<Use>,
    type_input: Use,
) -> TestResult {
    let ExampleBody::Captured {
        frame,
        field,
        source_type,
        target_guid,
    } = body.into_body()
    else {
        return Err("actual deferred field body".into());
    };
    let field = field.finish(&frame)?.into_cell(sources);
    let source_type = source_type.finish(&frame)?.into_cell(sources);
    let target_guid = target_guid.finish(&frame)?.into_cell(sources);
    assert_eq!(
        field.value().map(HealthText::as_str),
        Some(path),
        "the actual guard path remains complete"
    );
    exact_uses(&field, sources, field_inputs)?;
    assert_eq!(
        source_type.value().map(HealthText::as_str),
        Some(owner),
        "the actual owning concrete type"
    );
    exact_uses(&source_type, sources, vec![type_input])?;
    assert!(
        target_guid.value().is_none() && target_guid.origin().source_uses_refs().is_empty(),
        "the missing declaration has no invented Reference target datum"
    );
    Ok(())
}

fn exact_uses(
    cell: &SourcedValue<HealthText>,
    sources: &SourceUses,
    expected: Vec<Use>,
) -> TestResult {
    assert!(
        cell.origin().source_set_complete(),
        "all original contributors are closed"
    );
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        Some(false),
        "the fixture is a plain Archive source"
    );
    let inventory = sources.freeze()?;
    let mut actual = Vec::new();
    for id in cell.origin().source_uses_refs() {
        let used = inventory
            .uses()
            .iter()
            .find(|used| used.id() == id)
            .ok_or("actual retained source use")?;
        actual.push((used.location().clone(), used.role()));
    }
    assert_eq!(
        actual.len(),
        expected.len(),
        "the complete contributor cardinality is exact"
    );
    for expected in expected {
        assert_eq!(
            actual.iter().filter(|actual| **actual == expected).count(),
            1,
            "each exact original position and role occurs once"
        );
    }
    Ok(())
}
