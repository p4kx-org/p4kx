// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{Guid, StructValue, Value, ValueError};

use super::error::{self, Error, Kind};
use super::inventory::FieldKey;
use super::schema::Schema;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Observation {
    pub(crate) present: u64,
    pub(crate) populated: u64,
    pub(crate) read_errors: u64,
    pub(crate) missing_targets: u64,
}

pub(crate) type Observations = BTreeMap<(Guid, FieldKey), Observation>;

fn populated(value: &Value<'_>) -> bool {
    match value {
        Value::Null | Value::Error(_) => false,
        Value::Array(values) => !values.is_empty(),
        Value::String(value) | Value::Locale(value) => !value.is_empty(),
        _ => true,
    }
}

fn record(observation: &mut Observation, value: &Value<'_>) {
    observation.present += 1;
    observation.populated += u64::from(populated(value));
    match value {
        Value::Error(ValueError::DanglingReference { .. }) => observation.missing_targets += 1,
        Value::Error(_) => observation.read_errors += 1,
        _ => {}
    }
}

fn visit(visited: &mut usize, cancelled: &impl Fn() -> bool) -> super::Result<()> {
    error::check(cancelled)?;
    *visited += 1;
    if *visited > 1_000_000 {
        Err(Error::new(Kind::Bounds, "source values exceed their bound").into())
    } else {
        Ok(())
    }
}

pub(crate) fn observe(
    schema: &Schema,
    guid: Guid,
    root: &StructValue<'_>,
    observations: &mut Observations,
    cancelled: &impl Fn() -> bool,
) -> super::Result<()> {
    let mut pending = vec![(root, 0_usize)];
    let mut visited = 0_usize;
    while let Some((structure, depth)) = pending.pop() {
        error::check(cancelled)?;
        if depth > 64 {
            return Err(Error::new(Kind::Bounds, "source depth exceeds its bound").into());
        }
        for field in structure.fields() {
            visit(&mut visited, cancelled)?;
            let key = schema
                .owners
                .get(structure.type_name())
                .and_then(|fields| fields.get(field.name()));
            if let Some(key) = key {
                record(
                    observations.entry((guid, key.clone())).or_default(),
                    field.value(),
                );
            }
            match field.value() {
                Value::Struct(inner) => pending.push((inner, depth + 1)),
                Value::Array(values) => {
                    for value in values {
                        visit(&mut visited, cancelled)?;
                        if let Value::Struct(inner) = value {
                            pending.push((inner, depth + 1));
                        } else if let Some(key) = key {
                            let mut element = key.clone();
                            element.array_element = true;
                            if schema.declarations.contains_key(&element) {
                                record(observations.entry((guid, element)).or_default(), value);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
