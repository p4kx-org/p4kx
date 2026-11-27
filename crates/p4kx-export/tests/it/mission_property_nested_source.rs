// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Field, Value, ValueError};

use crate::health_origin_fixture::{Fixture, TestResult};
use crate::mission_property_nested_fixture::Case;
use crate::mission_property_sets_fixture::GENERATOR;

pub(crate) struct Original {
    pub(crate) source_type_ordinal: u64,
    pub(crate) failed_value_ordinal: Option<u64>,
}

pub(crate) fn check(input: &Fixture, case: Case) -> TestResult<Original> {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual Generator owner")?;
    assert_eq!(
        record.guid().to_string(),
        GENERATOR,
        "genuine original owner"
    );
    assert_eq!(
        record.type_name()?,
        "ContractGenerator",
        "genuine owner family"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the exact canonical tree belongs to this Record"
    );
    occurrences(&tree, case)
}

fn occurrences(tree: &p4kx_extraction::RecordTree<'_>, case: Case) -> TestResult<Original> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    let mut types = Vec::new();
    let mut errors = Vec::new();
    while let Some(value) = pending.pop() {
        match value {
            Value::Struct(value) => {
                if value.type_name() == case.source_type() {
                    types.push(ordinal);
                    if matches!(case, Case::UnreadableSearch) {
                        search(value)?;
                    }
                }
                pending.extend(value.fields().iter().rev().map(Field::value));
            }
            Value::Array(values) => pending.extend(values.iter().rev()),
            Value::Error(error) => {
                invalid_search_pointer(error, case);
                errors.push(ordinal);
            }
            _ => {}
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or("actual original ordinal overflow")?;
    }
    assert_eq!(
        types.len(),
        4,
        "handler, contract and two distinct SubContract occurrences"
    );
    let expected = if matches!(case, Case::UnreadableSearch) {
        4
    } else {
        0
    };
    assert_eq!(
        tree.error_count(),
        expected,
        "only the intended original decoder errors"
    );
    assert_eq!(
        errors.len(),
        expected,
        "the entire original source was checked before projection"
    );
    Ok(Original {
        source_type_ordinal: *types
            .first()
            .ok_or("genuine first selected concrete type")?,
        failed_value_ordinal: errors.first().copied(),
    })
}

fn invalid_search_pointer(error: &ValueError, case: Case) {
    assert!(
        matches!(case, Case::UnreadableSearch),
        "unsupported types are genuinely readable"
    );
    assert_eq!(
        *error,
        ValueError::InvalidPointer {
            struct_index: u32::MAX - 1,
            instance_index: 3,
        },
        "exact original error parameters"
    );
}

fn search(source: &p4kx_extraction::StructValue<'_>) -> TestResult {
    let field = source
        .fields()
        .iter()
        .find(|field| field.name() == "searchOptions")
        .ok_or("actual searchOptions declaration")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::StrongPointer,
        "actual authored pointer storage"
    );
    assert!(!field.storage().is_array(), "actual scalar pointer");
    assert_eq!(
        field.value(),
        &Value::Error(ValueError::InvalidPointer {
            struct_index: u32::MAX - 1,
            instance_index: 3,
        }),
        "present unreadable input is not absence"
    );
    Ok(())
}
