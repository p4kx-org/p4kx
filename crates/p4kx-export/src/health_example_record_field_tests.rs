// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::original::health_example_body::ExampleBodyReceipt;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldCapture;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

#[path = "health_example_record_field_assertions.rs"]
mod assertions;

use self::assertions::exact_body;

const LOCATION: &str = "nested.propertyOverrides";

fn bytes() -> Vec<u8> {
    let instance = || {
        TestValue::Struct(TestInstance::new("Params").value(
            "propertyOverrides",
            TestValue::Array(vec![TestValue::Bool(false)]),
        ))
    };
    let owner = || {
        TestRecord::new(0xEC13, "Owner", "Owner.Source", "original/source.xml")
            .value("flag", TestValue::Bool(true))
            .value("nested", instance())
            .value("other", instance())
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Owner")
                .field("flag", TestType::Bool)
                .field("nested", TestType::StrongPointer("Params".to_owned()))
                .field("other", TestType::StrongPointer("Params".to_owned())),
        )
        .structure(TestStruct::new("Params").array("propertyOverrides", TestType::Bool))
        .record(owner())
        .record(owner())
        .build()
}

fn params<'tree, 'source>(
    tree: &'tree p4kx_extraction::RecordTree<'source>,
    name: &str,
) -> TestResult<(
    &'tree p4kx_extraction::StructValue<'source>,
    &'tree Value<'source>,
)> {
    let Some(Value::Struct(parent)) = tree.field(name) else {
        return Err("actual Params Struct".into());
    };
    assert_eq!(
        parent.type_name(),
        "Params",
        "the nested source type differs from the Record header"
    );
    let value = parent
        .field("propertyOverrides")
        .ok_or("actual authored array")?;
    let Value::Array(values) = value else {
        return Err("actual original property array".into());
    };
    assert_eq!(
        values.as_ref(),
        [Value::Bool(false)],
        "the exact original false array is fully readable"
    );
    Ok((parent, value))
}

#[test]
fn record_field_recipe_keeps_the_dynamic_path_and_original_header_type() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual physical owner")?;
        let tree = record.tree()?;
        assert_eq!(tree.error_count(), 0, "the original is fully readable");
        assert_eq!(
            record.type_name()?,
            "Owner",
            "the independently read original Record header"
        );
        assert_eq!(
            tree.field("flag"),
            Some(&Value::Bool(true)),
            "the preceding original field"
        );
        let (parent, value) = params(&tree, "nested")?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let fact = fields.fact(value)?;
        assert_eq!(
            fact.ordinal(),
            2,
            "flag, Params, then its authored array in original DFS"
        );
        assert_eq!(
            fact.parent_ordinal(),
            Some(1),
            "the original Params occurrence owns this array"
        );
        assert_eq!(
            fact.field().name(),
            "propertyOverrides",
            "the exact actual owning declaration"
        );
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let other = forge.record(1).ok_or("actual second physical owner")?;
        assert_eq!(
            record.guid(),
            other.guid(),
            "the two real owner GUIDs intentionally coincide"
        );
        let other = RecordObservation::capture(input, &other)?;
        let before = scope.sources().freeze()?;
        let body = capture.record_field(parent, value, LOCATION)?;
        body.check_record_owner(admission.subject())?;
        body.check_description(Some(LOCATION), Some("Owner"), None)?;
        assert!(
            matches!(
                body.check_record_owner(other.subject()),
                Err(ExportAccountingError::ConflictingSourceDatum)
            ),
            "a different physical Record cannot own the same body"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "unretained collision recipes add no source uses"
        );
        exact_body(body, scope.sources())
    })
}

#[test]
fn record_field_rejects_wrong_path_equal_slot_and_equal_second_read_frame() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual physical owner")?;
        let tree = record.tree()?;
        let second = record.tree()?;
        assert_eq!(
            tree, second,
            "the independently decoded frames have equal visible values"
        );
        let (parent, value) = params(&tree, "nested")?;
        let (other, other_value) = params(&tree, "other")?;
        let (second_parent, second_value) = params(&second, "nested")?;
        assert_eq!(
            value, other_value,
            "equal original contents do not identify a selected slot"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let before = scope.sources().freeze()?;
        for result in [
            capture.record_field(parent, value, "other.propertyOverrides"),
            capture.record_field(other, other_value, LOCATION),
            capture.record_field(other, value, LOCATION),
            capture.record_field(second_parent, second_value, LOCATION),
        ] {
            assert!(
                matches!(
                    result,
                    Err(CatalogError::Accounting(
                        ExportAccountingError::ConflictingSourceDatum
                    ))
                ),
                "wrong source facts cannot mint an owner body"
            );
        }
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "failed admission materializes no contributions"
        );
        Ok(())
    })
}

#[test]
fn record_field_owner_check_rejects_foreign_input_even_with_shared_inventory_and_equal_headers()
-> TestResult {
    test_support::with_shared_inventory_inputs(
        &bytes(),
        |forge, input, other_forge, other_input| {
            let record = forge.record(0).ok_or("actual first owner")?;
            let other = other_forge.record(0).ok_or("actual foreign owner")?;
            assert_eq!(
                record.index(),
                other.index(),
                "equal original physical indices"
            );
            assert_eq!(record.guid(), other.guid(), "equal original nominal GUIDs");
            assert_eq!(
                record.record_path()?,
                other.record_path()?,
                "equal original path texts"
            );
            assert!(
                input
                    .origins()
                    .uses
                    .is_same_inventory(&other_input.origins().uses),
                "the negative control shares the actual report inventory"
            );
            let tree = record.tree()?;
            let (parent, value) = params(&tree, "nested")?;
            let scope = input.record_scope(&record, &tree)?;
            let fields = OriginalFlowFields::read(&scope, None)?;
            let admission = RecordObservation::capture(input, &record)?;
            let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
            let body = capture.record_field(parent, value, LOCATION)?;
            body.check_record_owner(&admission.subject().clone())?;
            let absent = ExampleBodyReceipt::absent(admission.subject().clone());
            assert!(
                matches!(
                    absent.check_record_owner(admission.subject()),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "an admitted header without an actual selected field cannot supply the owner body"
            );
            let foreign = RecordObservation::capture(other_input, &other)?;
            assert!(
                matches!(
                    body.check_record_owner(foreign.subject()),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "descriptor equality and one inventory cannot authorize a foreign original owner"
            );
            owner_registration(&tree, admission.subject(), foreign.subject(), &body)?;
            let mut clone = body.clone();
            clone.merge(body)?;
            Ok(())
        },
    )
}

fn projected_set<'source>(
    tree: &p4kx_extraction::RecordTree<'source>,
    location: &str,
) -> crate::mission_property_set::MissionPropertySet<'source> {
    use crate::mission_property_set::{MissionPropertyLevel, MissionPropertySet};
    MissionPropertySet {
        guid: crate::guid_text::CatalogGuid(tree.guid()),
        record_guid: crate::guid_text::CatalogGuid(tree.guid()),
        record_path: tree.record_path(),
        source_type: tree.type_name(),
        source_location: location.to_owned(),
        level: MissionPropertyLevel::Handler,
        properties: Vec::new(),
    }
}

fn owner_registration(
    tree: &p4kx_extraction::RecordTree<'_>,
    original: &crate::catalog_read::original::RecordSubject,
    foreign: &crate::catalog_read::original::RecordSubject,
    body: &ExampleBodyReceipt,
) -> TestResult {
    use crate::mission_property_set::MissionPropertySets;
    let mut rejected = MissionPropertySets::default();
    assert!(
        matches!(
            rejected.insert_subject(foreign.clone(), projected_set(tree, LOCATION), body.clone()),
            Err(CatalogError::Accounting(
                ExportAccountingError::UnboundHealthSubject
            ))
        ),
        "the real SetOwner constructor requires the actual body owner before inserting any row"
    );
    assert_eq!(
        rejected.into_rows().len(),
        0,
        "the rejected foreign admission leaves no canonical row"
    );
    let mut wrong = MissionPropertySets::default();
    assert!(
        matches!(
            wrong.insert_subject(
                original.clone(),
                projected_set(tree, "other.propertyOverrides"),
                body.clone()
            ),
            Err(CatalogError::Accounting(
                ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "the real SetOwner constructor rejects a wrong actual authored location"
    );
    assert_eq!(
        wrong.into_rows().len(),
        0,
        "the rejected location leaves no canonical row"
    );
    let mut own = MissionPropertySets::default();
    own.insert_subject(
        original.clone(),
        projected_set(tree, LOCATION),
        body.clone(),
    )?;
    own.insert_subject(
        original.clone(),
        projected_set(tree, LOCATION),
        body.clone(),
    )?;
    assert_eq!(
        own.ambiguous_owners().count(),
        0,
        "clones of the same genuine admission remain compatible"
    );
    assert_eq!(
        own.into_rows().len(),
        1,
        "the same source receipt remains canonically idempotent"
    );
    Ok(())
}
