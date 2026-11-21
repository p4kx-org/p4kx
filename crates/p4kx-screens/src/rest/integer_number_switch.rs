// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;

use p4kx_dataforge::{StructValue, Value};

use super::graph::Input;
use crate::bindings;
use crate::reader::{as_struct, exact};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum SwitchNumber {
    Constant(f64),
    Operation(usize),
}

type NumberChoice = Result<SwitchNumber, &'static str>;

#[derive(Debug, Clone, PartialEq)]
struct Case {
    key: f64,
    number: NumberChoice,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IntegerNumberSwitch {
    pub(super) input: Input,
    cases: Vec<Case>,
    default: NumberChoice,
}

impl IntegerNumberSwitch {
    pub(super) fn read(op: &StructValue<'_>, input: Input) -> Option<Self> {
        let Value::Array(values) = op.field("values")? else {
            return None;
        };
        let cases = values.iter().map(read_case).collect::<Option<_>>()?;
        Some(Self {
            input,
            cases,
            default: read_number(op, "defaultValue", "defaultOverride"),
        })
    }

    pub(super) fn select(&self, key: f64) -> NumberChoice {
        self.cases
            .iter()
            .find(|case| key.partial_cmp(&case.key).is_some_and(Ordering::is_eq))
            .map_or(self.default, |case| case.number)
    }
}

fn read_case(value: &Value<'_>) -> Option<Case> {
    let pair = as_struct(value)?;
    let first = pair.field("first")?;
    if matches!(first, Value::Single(_) | Value::Double(_)) {
        return None;
    }
    Some(Case {
        key: exact(first)?,
        number: read_number(pair, "second", "secondOverride"),
    })
}

fn read_number(structure: &StructValue<'_>, constant: &str, override_field: &str) -> NumberChoice {
    const INVALID_OVERRIDE: &str = "a number switch override that does not reference an operation";
    match structure.field(override_field) {
        None | Some(Value::Null) => structure
            .field(constant)
            .and_then(exact)
            .filter(|number| number.is_finite())
            .map(SwitchNumber::Constant)
            .ok_or("a number switch value that is missing or invalid"),
        Some(Value::Reference(reference)) => bindings::index_at(reference.location(), "operations")
            .map(SwitchNumber::Operation)
            .ok_or(INVALID_OVERRIDE),
        Some(_) => Err(INVALID_OVERRIDE),
    }
}
