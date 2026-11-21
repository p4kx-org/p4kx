// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::{RESTRICTIONS, TestResult};
use crate::real_archive_item_components::cell;
use crate::real_archive_item_durability::{self as proof, Trace};

pub(super) fn compare_type(source: &StructValue<'_>, row: &Value) -> TestResult {
    assert_eq!(
        proof::text(row, "/sourceType/value")?,
        source.type_name(),
        "the SDK row keeps the concrete DataForge declaration"
    );
    Ok(())
}

pub(super) fn compare_singles<'source>(
    source: &StructValue<'source>,
    row: &Value,
    names: &[&str],
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let mut count = 0;
    for &name in names {
        let datum = cell(row, name)?;
        super::super::single(source, name, datum)?;
        if let Some(raw @ Original::Single(_)) = source.field(name) {
            trace.check(datum, raw, "payload")?;
            count += 1;
        }
    }
    Ok(count)
}

pub(super) fn amount_field(value: &StructValue<'_>) -> Option<&'static str> {
    let field = match value.type_name() {
        "SCentiCargoUnit" => "centiSCU",
        "SMicroCargoUnit" => "microSCU",
        "SStandardCargoUnit" => "standardCargoUnits",
        _ => return None,
    };
    matches!(
        value.field(field),
        Some(Original::Int32(_) | Original::Single(_))
    )
    .then_some(field)
}

pub(super) fn trace_capacity<'source>(
    source: &StructValue<'source>,
    row: &Value,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let datum = cell(row, "capacity")?;
    match source.field("capacity") {
        Some(raw @ Original::Struct(unit)) => {
            trace.part(datum, raw, "$type", "selector")?;
            if let Some((field, value)) =
                amount_field(unit).and_then(|field| unit.field(field).map(|value| (field, value)))
            {
                trace.check(
                    cell(super::super::at(datum, "/value")?, field)?,
                    value,
                    "payload",
                )?;
                return Ok(1);
            }
        }
        Some(raw @ Original::Null) => trace.check(datum, raw, "selector")?,
        _ => {}
    }
    Ok(0)
}

pub(super) fn trace_links<'source>(
    source: &StructValue<'source>,
    row: &Value,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let mut count = 0;
    for &name in RESTRICTIONS {
        if let Some(raw @ Original::Array(slots)) = source.field(name) {
            let datum = cell(row, name)?;
            trace.check(datum, raw, "selector")?;
            for (source_slot, exported_slot) in slots.iter().zip(proof::rows(datum, "/value")?) {
                trace.check(exported_slot, source_slot, "selector")?;
                count += 1;
            }
        }
    }
    if let Some(raw @ Original::Array(slots)) = source.field("defaultComposition") {
        let datum = cell(row, "defaultComposition")?;
        trace.check(datum, raw, "selector")?;
        for (source_slot, exported_slot) in slots.iter().zip(proof::rows(datum, "/value")?) {
            count += trace_composition(exported_slot, source_slot, trace)?;
        }
    }
    Ok(count)
}

fn trace_composition<'source>(
    row: &Value,
    source: &Original<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let Original::Struct(entry) = source else {
        trace.check(row, source, "selector")?;
        return Ok(1);
    };
    trace.part(row, source, "$type", "selector")?;
    let payload = super::super::at(row, "/value")?;
    if let Some(raw) = entry.field("entry") {
        trace.check(cell(payload, "entry")?, raw, "selector")?;
    }
    if let Some(raw @ Original::Single(_)) = entry.field("weight") {
        trace.check(cell(payload, "weight")?, raw, "payload")?;
    }
    Ok(1)
}
