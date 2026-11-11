// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, StructValue, Value, ValueError};

use crate::health_origin_fixture::{Fixture, TestResult};
use crate::mission_property_remaining_fixture::{Case, NAN_BITS};
use crate::mission_property_sets_fixture::GENERATOR;

pub(crate) type Coordinate = (u64, &'static str, &'static str);

pub(crate) struct Original {
    pub(crate) source_type_ordinal: u64,
    pub(crate) field_uses: Vec<Coordinate>,
}

pub(crate) fn check(input: &Fixture, case: Case) -> TestResult<Original> {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual Generator")?;
    assert_eq!(
        record.guid().to_string(),
        GENERATOR,
        "genuine physical owner"
    );
    assert_eq!(
        record.type_name()?,
        "ContractGenerator",
        "actual owner family"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the actual owned tree belongs to this original Record"
    );
    scan(&tree, case, 4)
}

pub(crate) fn check_broker(input: &Fixture, case: Case) -> TestResult<Original> {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual physical Broker owner")?;
    assert_eq!(
        record.guid().to_string(),
        crate::health_origin_fixture::guid(crate::scoped_fixture::BROKER),
        "genuine selected Broker GUID"
    );
    assert_eq!(
        record.type_name()?,
        "MissionBrokerEntry",
        "actual selected Broker family"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "same genuine immutable Record/tree pair"
    );
    scan(&tree, case, 1)
}

#[derive(Default)]
struct Seen<'tree, 'source> {
    selected: Option<&'tree Value<'source>>,
    types: Vec<u64>,
    field_uses: Option<Vec<Coordinate>>,
    errors: usize,
}

impl<'tree, 'source> Seen<'tree, 'source> {
    fn observe(
        &mut self,
        value: &'tree Value<'source>,
        ordinal: u64,
        path: &[Coordinate],
        case: Case,
    ) -> TestResult {
        if self
            .selected
            .is_some_and(|selected| std::ptr::eq(value, selected))
        {
            self.field_uses = Some(path.to_vec());
        }
        match value {
            Value::Struct(source) if source.type_name() == case.source_type() => {
                self.types.push(ordinal);
                let actual = declaration(source, case)?;
                if self.selected.is_none() {
                    self.selected = actual;
                }
            }
            Value::Error(error) => {
                assert_eq!(
                    *error,
                    ValueError::InvalidBool { byte: 3 },
                    "exact original malformed byte"
                );
                self.errors = self.errors.checked_add(1).ok_or("original error count")?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn scan(tree: &p4kx_extraction::RecordTree<'_>, case: Case, count: usize) -> TestResult<Original> {
    let mut seen = Seen::default();
    let mut pending: Vec<_> = tree
        .fields()
        .iter()
        .rev()
        .map(|field| (field.value(), true, Vec::new()))
        .collect();
    let mut ordinal = 0_u64;
    while let Some((value, field, mut path)) = pending.pop() {
        if field {
            path.push((ordinal, "$fieldName", "conversion"));
        }
        if matches!(value, Value::Array(_)) {
            path.push((ordinal, "$value", "selector"));
        }
        seen.observe(value, ordinal, &path, case)?;
        match value {
            Value::Struct(source) => pending.extend(
                source
                    .fields()
                    .iter()
                    .rev()
                    .map(|field| (field.value(), true, path.clone())),
            ),
            Value::Array(values) => pending.extend(
                values
                    .iter()
                    .rev()
                    .map(|value| (value, false, path.clone())),
            ),
            _ => {}
        }
        ordinal = ordinal.checked_add(1).ok_or("original ordinal")?;
    }
    finish(
        case,
        tree.error_count(),
        seen.errors,
        &seen.types,
        seen.field_uses,
        count,
    )
}

fn declaration<'tree, 'source>(
    source: &'tree StructValue<'source>,
    case: Case,
) -> TestResult<Option<&'tree Value<'source>>> {
    let Some(name) = case.stored_field() else {
        return Ok(None);
    };
    let field = source
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .ok_or("genuine selected original declaration")?;
    assert!(
        !field.storage().is_array(),
        "the original failing slot is scalar"
    );
    match case {
        Case::Investigation | Case::Name | Case::Option => {
            assert_eq!(
                field.storage().data_type(),
                DataType::Bool,
                "original Bool declaration"
            );
            assert_eq!(
                field.value(),
                &Value::Error(ValueError::InvalidBool { byte: 3 }),
                "the actual source decoder retained its malformed byte"
            );
        }
        Case::Race | Case::Organization => {
            assert_eq!(
                field.storage().data_type(),
                DataType::Single,
                "original Single declaration"
            );
            let Value::Single(value) = field.value() else {
                return Err("actual original Single".into());
            };
            assert_eq!(
                value.to_bits(),
                NAN_BITS,
                "exact authored IEEE width and bits"
            );
        }
        Case::Clue | Case::MissionItem | Case::Spawn => {
            return Err("inapplicable field control".into());
        }
    }
    Ok(Some(field.value()))
}

fn finish(
    case: Case,
    tree_errors: usize,
    errors: usize,
    types: &[u64],
    field_uses: Option<Vec<Coordinate>>,
    count: usize,
) -> TestResult<Original> {
    assert_eq!(
        types.len(),
        count,
        "all actual physical property occurrences selected by this source fixture"
    );
    let expected = usize::from(matches!(
        case,
        Case::Investigation | Case::Name | Case::Option
    )) * count;
    assert_eq!(
        errors, expected,
        "the full source has exactly the intended errors"
    );
    assert_eq!(
        tree_errors, expected,
        "the independent canonical error count agrees"
    );
    let field_uses = match (case.stored_field(), field_uses) {
        (Some(_), Some(uses)) => uses,
        (None, None) => Vec::new(),
        _ => return Err("actual field occurrence was not reached in the original DFS".into()),
    };
    Ok(Original {
        source_type_ordinal: *types.first().ok_or("first genuine source type")?,
        field_uses,
    })
}
