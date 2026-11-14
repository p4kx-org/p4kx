// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Field, Record, Reference, StructValue, Value, ValueError};

use crate::flow_action_fixture::{self as fixture, Case};
use crate::health_origin_fixture::TestResult;

pub(crate) fn check(value: &StructValue<'_>, case: Case, later: bool) -> TestResult {
    assert_eq!(value.type_name(), "FlowTarget", "real owned target type");
    if matches!(case, Case::Absent) {
        assert!(
            value.fields().is_empty(),
            "id/variable are genuinely undeclared, not null"
        );
        return Ok(());
    }
    assert_eq!(
        value.fields().len(),
        2,
        "both actual target fields are visited"
    );
    let id = value
        .fields()
        .first()
        .ok_or("actual target id declaration")?;
    let variable = value
        .fields()
        .get(1)
        .ok_or("actual target variable declaration")?;
    assert_eq!(id.name(), "id", "real selected field name");
    assert_eq!(
        variable.name(),
        "missionVariableName",
        "real selected variable field name"
    );
    match case {
        Case::Null => null_values(id, variable),
        Case::WrongShape => {
            assert_eq!(
                id.storage().data_type(),
                DataType::Bool,
                "readable wrong id type"
            );
            assert_eq!(
                variable.storage().data_type(),
                DataType::Bool,
                "readable wrong variable type"
            );
            assert_eq!(
                id.value(),
                &Value::Bool(true),
                "the original wrong-shaped true is retained"
            );
            assert_eq!(
                variable.value(),
                &Value::Bool(false),
                "the original wrong-shaped false is retained"
            );
        }
        Case::Unreadable => errors(value)?,
        _ => values(value, case, later)?,
    }
    Ok(())
}

fn values(target: &StructValue<'_>, case: Case, later: bool) -> TestResult {
    let later = later && !matches!(case, Case::EqualValues);
    let id = target.fields().first().ok_or("real id field")?;
    let variable = target.fields().get(1).ok_or("real variable field")?;
    assert_eq!(
        id.storage().data_type(),
        DataType::Guid,
        "original GUID storage"
    );
    assert_eq!(
        variable.storage().data_type(),
        DataType::String,
        "original String storage"
    );
    let Some(Value::Guid(value)) = target.field("id") else {
        return Err("actual original GUID value".into());
    };
    let expected = if matches!(case, Case::NilEmpty) {
        "00000000-0000-0000-0000-000000000000"
    } else if later {
        fixture::AFTER
    } else {
        fixture::BEFORE
    };
    assert_eq!(
        value.to_string(),
        expected,
        "selected instance field differs from owner header GUID"
    );
    assert_ne!(
        value.to_string(),
        fixture::OWNER,
        "target field cannot be replaced by the owning Record header"
    );
    let expected = if matches!(case, Case::NilEmpty) {
        ""
    } else if later {
        fixture::LATE_VARIABLE
    } else {
        fixture::EARLY_VARIABLE
    };
    assert_eq!(
        target.field("missionVariableName"),
        Some(&Value::String(expected)),
        "exact empty/whitespace/source variable text"
    );
    Ok(())
}

fn errors(target: &StructValue<'_>) -> TestResult {
    let id = target.fields().first().ok_or("real malformed id field")?;
    let variable = target
        .fields()
        .get(1)
        .ok_or("real malformed variable field")?;
    assert_eq!(
        id.storage().data_type(),
        DataType::Bool,
        "actual malformed Bool storage"
    );
    assert_eq!(
        variable.storage().data_type(),
        DataType::String,
        "actual unreadable String storage"
    );
    assert_eq!(
        id.value(),
        &Value::Error(ValueError::InvalidBool { byte: 129 }),
        "all original malformed Bool bits"
    );
    assert_eq!(
        variable.value(),
        &Value::Error(ValueError::InvalidString {
            table: "text",
            offset: u32::MAX - 2
        }),
        "original invalid String table and offset"
    );
    Ok(())
}

pub(crate) fn reference<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    name: &str,
    record: &Record<'_>,
    position: u64,
) -> TestResult<&'tree Reference<'source>> {
    let field = parent
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .ok_or("actual original pointer declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::WeakPointer,
        "actual weak-slot storage"
    );
    let Value::Reference(reference) = field.value() else {
        return Err("actual resolved original WeakPointer".into());
    };
    let receipt = reference.original_target();
    assert!(
        receipt.shares_source_with(record),
        "actual same-Forge original target admission"
    );
    assert_eq!(
        receipt.record_index(),
        record.index(),
        "actual same original owning Record"
    );
    assert_eq!(
        receipt.instance_positions(),
        Some(&[position][..]),
        "precise original instance field ordinal"
    );
    assert_eq!(
        receipt.record_type(),
        None,
        "instance target has no fake Record type"
    );
    assert_eq!(
        reference.type_name(),
        "FlowTarget",
        "actual selected instance type"
    );
    Ok(reference)
}

fn null_values(id: &Field<'_>, variable: &Field<'_>) {
    assert_eq!(
        id.storage().data_type(),
        DataType::StrongPointer,
        "real null storage differs from GUID0"
    );
    assert_eq!(
        variable.storage().data_type(),
        DataType::StrongPointer,
        "real null variable storage"
    );
    assert_eq!(id.value(), &Value::Null, "genuine original null id value");
    assert_eq!(
        variable.value(),
        &Value::Null,
        "genuine original null variable value"
    );
}
