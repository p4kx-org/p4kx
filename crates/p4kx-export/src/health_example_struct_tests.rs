// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole, SourcedValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::ExampleBodyCapture;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_read::original::health_example_body::ExampleBody;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, HealthText};

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Owner")
                .field("flag", TestType::Bool)
                .field("nested", TestType::StrongPointer("Mystery".to_owned())),
        )
        .structure(TestStruct::new("Mystery").field("payload", TestType::String))
        .record(
            TestRecord::new(0xEC12, "Owner", "Owner.Source", "original/source.xml")
                .value("flag", TestValue::Bool(true))
                .value(
                    "nested",
                    TestValue::Struct(
                        TestInstance::new("Mystery").value("payload", TestValue::text("kept")),
                    ),
                ),
        )
        .build()
}

#[test]
fn unsupported_struct_and_record_types_use_their_exact_original_coordinates() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        assert_eq!(tree.error_count(), 0, "actual fully readable original");
        assert_eq!(
            tree.field("flag"),
            Some(&Value::Bool(true)),
            "original first field"
        );
        let Some(Value::Struct(nested)) = tree.field("nested") else {
            return Err("actual original nested Struct".into());
        };
        assert_eq!(
            nested.type_name(),
            "Mystery",
            "actual selected concrete type"
        );
        assert_eq!(
            nested.field("payload"),
            Some(&Value::String("kept")),
            "original nested payload"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let fact = fields
            .owner_of(nested)?
            .ok_or("actual nested owner occurrence")?;
        assert_eq!(
            fact.ordinal(),
            1,
            "DFS original Struct occurrence follows flag"
        );
        assert_eq!(
            fact.parent_ordinal(),
            None,
            "nested is an actual root field"
        );
        assert_eq!(fact.field().name(), "nested", "actual declaration name");
        let admission = RecordObservation::capture(input, &record)?;
        let capture = ExampleBodyCapture::new(input, &admission, &scope)?;
        let before = scope.sources().freeze()?;
        let nested = capture.struct_fact(fact, &fields.ancestors(fact)?, "nested")?;
        let root = capture.record_type()?;
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "unretained recipes make no source claims"
        );
        check_nested(nested, scope.sources())?;
        check_root(root, scope.sources())?;
        Ok(())
    })
}

fn check_nested(
    nested: crate::catalog_read::original::ExampleBodyReceipt,
    sources: &p4kx_extraction::SourceUses,
) -> TestResult {
    let ExampleBody::Captured {
        frame,
        field,
        source_type,
        target_guid,
    } = nested.into_body()
    else {
        return Err("actual deferred Struct body".into());
    };
    let field = field.finish(&frame)?.into_cell(sources);
    let source_type = source_type.finish(&frame)?.into_cell(sources);
    let target = target_guid.finish(&frame)?.into_cell(sources);
    assert_eq!(
        field.value().map(HealthText::as_str),
        Some("nested"),
        "actual complete path"
    );
    assert_eq!(
        source_type.value().map(HealthText::as_str),
        Some("Mystery"),
        "this Struct type"
    );
    check_one(
        &field,
        sources,
        "dataforgeValue",
        &[0, 1],
        "$fieldName",
        SourceUseRole::Conversion,
    )?;
    check_one(
        &source_type,
        sources,
        "dataforgeValue",
        &[0, 1],
        "$type",
        SourceUseRole::Selector,
    )?;
    assert!(
        target.value().is_none() && target.origin().source_uses_refs().is_empty(),
        "this operation has no target GUID contribution"
    );
    Ok(())
}

fn check_root(
    root: crate::catalog_read::original::ExampleBodyReceipt,
    sources: &p4kx_extraction::SourceUses,
) -> TestResult {
    let ExampleBody::Captured {
        frame,
        field,
        source_type,
        ..
    } = root.into_body()
    else {
        return Err("actual deferred Record body".into());
    };
    let field = field.finish(&frame)?.into_cell(sources);
    let source_type = source_type.finish(&frame)?.into_cell(sources);
    assert_eq!(
        field.value().map(HealthText::as_str),
        Some(""),
        "root operation has an empty control path"
    );
    assert!(
        field.origin().source_uses_refs().is_empty() && field.origin().source_set_complete(),
        "root path control does not pretend to read an authored field name"
    );
    assert_eq!(
        source_type.value().map(HealthText::as_str),
        Some("Owner"),
        "actual owning header type"
    );
    check_one(
        &source_type,
        sources,
        "dataforgeRecordHeader",
        &[0],
        "sourceType",
        SourceUseRole::Selector,
    )
}

fn check_one(
    cell: &SourcedValue<HealthText>,
    sources: &p4kx_extraction::SourceUses,
    namespace: &str,
    indices: &[u64],
    field: &str,
    role: SourceUseRole,
) -> TestResult {
    assert_eq!(
        cell.origin().source_uses_refs().len(),
        1,
        "exactly one actual contributing fact"
    );
    assert!(
        cell.origin().source_set_complete(),
        "the genuine contribution set is complete"
    );
    assert_eq!(
        cell.origin().contains_encrypted_source(),
        Some(false),
        "actual plain original input"
    );
    let id = cell
        .origin()
        .source_uses_refs()
        .first()
        .ok_or("one actual contributor")?;
    let inventory = sources.freeze()?;
    let item = inventory
        .uses()
        .iter()
        .find(|item| item.id() == id)
        .ok_or("genuine emitted source use")?;
    assert_eq!(item.role(), role, "the exact actual original read role");
    assert_eq!(
        item.location(),
        &SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices: indices.to_vec(),
            field: field.to_owned(),
        },
        "the exact original physical coordinate"
    );
    Ok(())
}

#[test]
fn unsupported_struct_fact_rejects_a_scalar_and_an_equal_second_read_frame() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original owner")?;
        let tree = record.tree()?;
        let second = record.tree()?;
        assert!(
            tree.is_from(&record) && second.is_from(&record),
            "both genuine reads have one physical owner"
        );
        assert_eq!(tree, second, "all visible original values coincide");
        let scope = input.record_scope(&record, &tree)?;
        let other_scope = input.record_scope(&record, &second)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let other = OriginalFlowFields::read(&other_scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = ExampleBodyCapture::new(input, &admission, &scope)?;
        let scalar = fields.fact(tree.field("flag").ok_or("actual scalar")?)?;
        assert_eq!(
            scalar.value(),
            &Value::Bool(true),
            "actual first original is a scalar"
        );
        let foreign = other.fact(second.field("nested").ok_or("actual second-read Struct")?)?;
        assert_eq!(
            foreign.ordinal(),
            1,
            "equal descriptor retains the same original ordinal"
        );
        let before = scope.sources().freeze()?;
        for receipt in [
            capture.struct_fact(scalar, &fields.ancestors(scalar)?, "flag"),
            capture.struct_fact(foreign, &other.ancestors(foreign)?, "nested"),
        ] {
            assert!(
                matches!(
                    receipt,
                    Err(CatalogError::Accounting(
                        ExportAccountingError::ConflictingSourceDatum
                    ))
                ),
                "a scalar or another actual read cannot supply this Struct receipt"
            );
        }
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "invalid frames materialize no contributions"
        );
        Ok(())
    })
}
