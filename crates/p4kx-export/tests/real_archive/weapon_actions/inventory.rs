// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{StructValue, Value as Original, ValueError};
use serde_json::Value;

use super::TestResult;
use crate::original_item_arena;
use crate::real_archive_item_components::cell;
use crate::real_archive_item_durability::Trace;

pub(super) fn remaining<'source, 'wire>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    arena: &Value,
    mut projected: impl FnMut(&str) -> Option<&'wire Value>,
) -> TestResult<usize> {
    let mut consumed = HashSet::new();
    let remaining = source.fields().iter().filter(|field| {
        let name = field.name();
        !projected(name).is_some_and(materialized) || !consumed.insert(name)
    });
    Ok(original_item_arena::compare(trace, remaining, arena)?.nodes)
}

pub(super) fn authored<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    body: &Value,
) -> TestResult<usize> {
    remaining(trace, source, cell(body, "authored")?, |name| {
        body.get(name)
    })
}

pub(super) fn damage_macro<'source>(
    trace: &Trace<'_, 'source>,
    original: Option<&Original<'source>>,
    cell: &Value,
) -> TestResult<usize> {
    crate::real_archive_vehicle_armor::compare_macro(original, cell)?;
    let Some(raw) = original else {
        return Ok(0);
    };
    match raw {
        Original::Error(
            ValueError::DanglingReference { .. } | ValueError::UnexpectedReferenceType { .. },
        ) => {
            trace.part(cell, raw, "$error.guid", "selector")?;
        }
        _ => trace.check(cell, raw, "selector")?,
    }
    Ok(1)
}

fn materialized(cell: &Value) -> bool {
    matches!(
        cell.get("state").and_then(Value::as_str),
        Some("present" | "null")
    )
}
