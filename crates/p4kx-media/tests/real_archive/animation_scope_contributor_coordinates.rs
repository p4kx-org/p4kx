// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::str::FromStr;

use serde_json::Value;

use super::TestResult;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Site {
    pub(super) entry: usize,
    pub(super) element: u64,
    pub(super) declaration: u64,
}

impl Site {
    pub(super) fn of(value: &Value, pointer: &str) -> TestResult<Self> {
        let item = value.pointer(pointer).ok_or("original coordinate object")?;
        Ok(Self {
            entry: number(item, "/entry_index")?,
            element: number(item, "/element_ordinal")?,
            declaration: number(item, "/declaration_ordinal")?,
        })
    }
}

pub(super) fn referenced_entry(value: &Value) -> TestResult<Option<usize>> {
    (string(value, "/reference/resolution/status")? == "resolved")
        .then(|| {
            entry_number(
                value,
                "/reference/resolution/target/source/entry/entry_index",
            )
        })
        .transpose()
}

pub(super) fn entry_number(value: &Value, pointer: &str) -> TestResult<usize> {
    let numeric = value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or("numeric directory index")?;
    Ok(usize::try_from(numeric)?)
}

pub(super) fn number<T: FromStr>(value: &Value, pointer: &str) -> TestResult<T> {
    string(value, pointer)?
        .parse()
        .map_err(|_| format!("invalid physical coordinate {pointer}").into())
}

pub(super) fn string<'a>(value: &'a Value, pointer: &str) -> TestResult<&'a str> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing authored value {pointer}").into())
}
