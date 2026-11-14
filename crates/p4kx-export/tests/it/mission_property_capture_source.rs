// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Field, StructValue, Value, ValueError, ValueKind};

use crate::health_origin_fixture::{Fixture, TestResult};
use crate::mission_property_capture_fixture::Case;
use crate::mission_property_sets_fixture::GENERATOR;

pub(crate) fn check(input: &Fixture, case: Case) -> TestResult<Option<u64>> {
    let forge = input.entry.open()?;
    assert_eq!(forge.records().len(), 1, "one genuine physical owner");
    let record = forge.record(0).ok_or("original Generator")?;
    assert_eq!(record.guid().to_string(), GENERATOR, "original owner GUID");
    assert_eq!(
        record.type_name()?,
        "ContractGenerator",
        "original owner family"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "canonical tree belongs to this actual Record"
    );
    let params = parameters(&tree)?;
    let declaration = params
        .fields()
        .first()
        .ok_or("original propertyOverrides")?;
    assert_eq!(
        declaration.name(),
        "propertyOverrides",
        "actual selected declaration"
    );
    assert_eq!(
        declaration.storage().data_type(),
        DataType::StrongPointer,
        "authored slot type"
    );
    assert!(
        declaration.storage().is_array(),
        "authored overrides are an array"
    );
    let value = declaration.value();
    match case {
        Case::Empty => {
            assert_eq!(
                tree.error_count(),
                0,
                "readable empty source has no hidden error"
            );
            let Value::Array(values) = value else {
                return Err("original empty array".into());
            };
            assert!(values.is_empty(), "genuine present empty array");
            Ok(None)
        }
        Case::OverrideArray => error(
            &tree,
            value,
            ValueError::ArrayOutOfRange {
                first: u32::MAX,
                count: 1,
                len: u64::try_from(forge.value_count(ValueKind::StrongPointer))?,
                pool: "strong pointer pool",
            },
        ),
        Case::PropertyPointer => pointer(&tree, value),
    }
}

fn parameters<'tree, 'source>(
    tree: &'tree p4kx_extraction::RecordTree<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    let root = tree
        .fields()
        .first()
        .ok_or("original generators declaration")?;
    assert_eq!(root.name(), "generators", "original root selection");
    assert_eq!(
        root.storage().data_type(),
        DataType::StrongPointer,
        "original pointer storage"
    );
    assert!(root.storage().is_array(), "original handler array");
    let handler = first_struct(root.value())?;
    assert_eq!(
        handler.type_name(),
        "ContractGeneratorHandler_Career",
        "actual selected handler"
    );
    structure(
        handler
            .field("contractParams")
            .ok_or("original contractParams")?,
    )
}

fn pointer(tree: &p4kx_extraction::RecordTree<'_>, array: &Value<'_>) -> TestResult<Option<u64>> {
    let property = first_struct(array)?;
    assert_eq!(
        property.field("missionVariableName"),
        Some(&Value::String("Broken")),
        "original variable survives independently of its unreadable value"
    );
    assert_eq!(
        property.field("extendedTextToken"),
        Some(&Value::String("Preserved")),
        "original token survives independently of its unreadable value"
    );
    let value = property
        .fields()
        .iter()
        .find(|field| field.name() == "value")
        .ok_or("original property value declaration")?;
    assert_eq!(
        value.storage().data_type(),
        DataType::StrongPointer,
        "actual value storage"
    );
    assert!(
        !value.storage().is_array(),
        "actual value is a scalar pointer"
    );
    error(
        tree,
        value.value(),
        ValueError::InvalidPointer {
            struct_index: u32::MAX - 1,
            instance_index: 3,
        },
    )
}

fn error(
    tree: &p4kx_extraction::RecordTree<'_>,
    value: &Value<'_>,
    expected: ValueError,
) -> TestResult<Option<u64>> {
    assert_eq!(
        value,
        &Value::Error(expected),
        "all actual original error parameters"
    );
    assert_eq!(
        tree.error_count(),
        1,
        "exactly the intended unreadable occurrence"
    );
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    let mut selected = None;
    while let Some(current) = pending.pop() {
        if std::ptr::eq(current, value) {
            selected = Some(ordinal);
        }
        ordinal = ordinal.checked_add(1).ok_or("source ordinal overflow")?;
        match current {
            Value::Struct(value) => pending.extend(value.fields().iter().rev().map(Field::value)),
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    assert!(
        selected.is_some(),
        "actual failed occurrence belongs to the whole original DFS"
    );
    Ok(selected)
}

fn first_struct<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    let Value::Array(values) = value else {
        return Err("original source array".into());
    };
    structure(values.first().ok_or("original first array element")?)
}

fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Value::Struct(value) => Ok(value),
        _ => Err("original source struct".into()),
    }
}
