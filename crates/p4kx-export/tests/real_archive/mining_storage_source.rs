// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::CatalogSummary;
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, InputScope, RecordTree, SourceEncryption, SourceInput,
    SourceUses, Value as Original,
};
use serde::Deserialize;
use serde_json::Value;

use super::{TestResult, index, text};
use crate::catalog_v3_consumer::present;
use crate::fuel_capacity::compare_capacity;
use crate::real_archive_cargo_grid_capacity::selected_uses;
use crate::real_archive_item_durability::Trace;

const CONTAINER: &str = "ResourceContainer";

pub(super) fn compare(
    entry: &DataForgeEntry,
    pod: &RecordTree<'_>,
    facilities: &[Value],
    containers: &[Value],
    summary: &CatalogSummary,
) -> TestResult<usize> {
    let original = original_containers(pod)?;
    let record_index = u64::from(pod.record_index());
    let owns = |row: &Value| index(row, "/ownerRecordIndex/value") == Some(record_index);
    if facilities
        .iter()
        .any(|row| owns(row) && text(row, "/kind") == Some("cargoGrid"))
    {
        return Err("the pod must not publish a cargoGrid facility".into());
    }
    let rows: Vec<_> = containers.iter().filter(|&row| owns(row)).collect();
    if rows.len() != original.len() {
        return Err("the number of physical pod resource container slots differs".into());
    }
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("expected a physical DataForge Archive Entry".into());
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let marker = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let recorded = selected_uses(summary, &rows)?;
    let uses = recorded
        .iter()
        .map(|row| Ok((text(row, "/id").ok_or("selected SourceUse ID")?, row)))
        .collect::<TestResult<HashMap<_, _>>>()?;
    let trace = Trace::of(pod, &uses, &scope, marker);
    let guid = pod.guid().to_string();
    for (slot, component) in original {
        let mut matching = rows
            .iter()
            .copied()
            .filter(|row| index(row, "/componentOrdinal/value") == u64::try_from(slot).ok());
        let row = matching
            .next()
            .ok_or("original pod resource container slot")?;
        if matching.next().is_some() || text(row, "/ownerGuid/value") != Some(guid.as_str()) {
            return Err("the resource container must identify one physical pod slot".into());
        }
        check_capacity(&trace, component, row)?;
    }
    Ok(rows.len())
}

fn original_containers<'read, 'source>(
    pod: &'read RecordTree<'source>,
) -> TestResult<Vec<(usize, &'read Original<'source>)>> {
    let Some(Original::Array(components)) = pod.field("Components") else {
        return Err("original pod Components array".into());
    };
    let mut resource = Vec::new();
    for (slot, component) in components.iter().enumerate() {
        match component {
            Original::Struct(value) => match value.type_name() {
                CONTAINER => resource.push((slot, component)),
                "SCItemCargoGridParams" | "SCItemInventoryContainerComponentParams" => {
                    return Err(
                        "the nominated pod declares an ordinary cargo or storage grid".into(),
                    );
                }
                _ => {}
            },
            Original::Null => {}
            _ => return Err("the original pod has an unmeasured component slot".into()),
        }
    }
    if resource.is_empty() {
        return Err("the nominated pod needs a physical ResourceContainer".into());
    }
    Ok(resource)
}

fn check_capacity<'source>(
    trace: &Trace<'_, 'source>,
    original: &Original<'source>,
    row: &Value,
) -> TestResult {
    let Original::Struct(component) = original else {
        return Err("original resource container component".into());
    };
    let type_cell = row.get("sourceType").ok_or("container source type")?;
    if present(type_cell)?.as_str() != Some(CONTAINER) {
        return Err("the resource container's original nominal type differs".into());
    }
    trace.part(type_cell, original, "$type", "selector")?;
    trace.check(
        row.get("componentOrdinal").ok_or("source component slot")?,
        original,
        "conversion",
    )?;
    trace.header(
        row.get("ownerRecordIndex")
            .ok_or("physical owner selector")?,
        "recordIndex",
        "selector",
    )?;
    let Some(raw @ Original::Struct(unit)) = component.field("capacity") else {
        return Err("nominated mining storage requires a concrete capacity unit".into());
    };
    let cell = row.get("capacity").ok_or("source capacity")?;
    let value = present(cell)?;
    compare_capacity(Some(raw), cell)?;
    trace.part(cell, raw, "$type", "selector")?;
    trace.part(
        value
            .get("sourceType")
            .ok_or("original unit type carrier")?,
        raw,
        "$type",
        "selector",
    )?;
    check_amount(trace, unit, value)
}

fn check_amount<'source>(
    trace: &Trace<'_, 'source>,
    unit: &p4kx_extraction::StructValue<'source>,
    value: &Value,
) -> TestResult {
    let (kind, field) = match unit.type_name() {
        "SStandardCargoUnit" => ("standardCargoUnit", "standardCargoUnits"),
        "SCentiCargoUnit" => ("centiCargoUnit", "centiSCU"),
        "SMicroCargoUnit" => ("microCargoUnit", "microSCU"),
        _ => return Err("a recognized original positive capacity unit is required".into()),
    };
    let amount = unit.field(field).ok_or("original unit amount")?;
    let positive = match amount {
        Original::Single(value) => value.is_finite() && *value > 0.0,
        Original::Int32(value) => *value > 0,
        _ => false,
    };
    if !positive || text(value, "/kind") != Some(kind) {
        return Err("the mining pod requires a matching positive capacity".into());
    }
    let amount_cell = value.get(field).ok_or("source amount carrier")?;
    present(amount_cell)?;
    trace.check(amount_cell, amount, "payload")?;
    if let Original::Single(expected) = amount {
        let finite = amount_cell
            .pointer("/value/finite")
            .ok_or("finite Single view")?;
        if f32::deserialize(finite)?.to_bits() != expected.to_bits() {
            return Err("the finite Single must match its original value".into());
        }
    }
    Ok(())
}
