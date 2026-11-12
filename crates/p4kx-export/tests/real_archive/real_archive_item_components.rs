// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::{Kind, KindRule, is_kind_leftover};
use p4kx_extraction::{
    Filter, PointerProbe, Record, RecordTree, StructValue, TreeBuilder, Value as Original,
};
use serde_json::Value;

use crate::real_archive_item_durability::{self as proof, Trace};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(super) type Rows<'catalog> = BTreeMap<(u32, usize), &'catalog Value>;
pub(super) const FILTER: &str = "libs/foundry/records/entities/*";

pub(super) fn selected_item(
    record: &Record<'_>,
    rule: &KindRule,
    filter: &Filter,
) -> TestResult<bool> {
    let path = record
        .record_path()
        .map_err(|_| "read original Record path")?;
    if !filter.matches(path) || !rule.matches_path(path) {
        return Ok(false);
    }
    let source_type = record
        .type_name()
        .map_err(|_| "read original Record type")?;
    let name = record.name().map_err(|_| "read original Record name")?;
    Ok(source_type == rule.type_name && !is_kind_leftover(Kind::Items, path, name))
}

pub(super) fn has_component<'source>(
    trees: &mut TreeBuilder<'source>,
    record: &Record<'source>,
    probes: &[PointerProbe<'source>],
) -> TestResult<bool> {
    for probe in probes {
        if trees
            .holds(probe, record)
            .map_err(|_| "inspect original Components array")?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn component_rows<'catalog>(
    catalog: &'catalog Value,
    path: &str,
) -> TestResult<Rows<'catalog>> {
    let mut rows = Rows::new();
    let mut previous = None;
    for row in proof::rows(catalog, path)? {
        let owner = u32::try_from(proof::number(row, "/ownerRecordIndex/value")?)?;
        let slot = usize::try_from(proof::number(row, "/componentOrdinal/value")?)?;
        let position = (owner, slot);
        if previous.is_some_and(|key| key >= position) || rows.insert(position, row).is_some() {
            return Err("exported component slots must be unique and follow physical order".into());
        }
        previous = Some(position);
    }
    Ok(rows)
}

pub(super) fn compare_identity<'source>(
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
    original: &Original<'source>,
    component: &StructValue<'source>,
    kind: &str,
    row: &Value,
) -> TestResult {
    assert!(
        proof::text(row, "/ownerGuid/value")? == tree.guid().to_string(),
        "the exported owner is the original physical Record"
    );
    assert!(
        proof::text(row, "/sourceType/value")? == component.type_name()
            && proof::text(row, "/configuration/value/type")? == kind,
        "the exported shape follows the original concrete component type"
    );
    trace.header(cell(row, "ownerGuid")?, "guid", "payload")?;
    trace.header(cell(row, "ownerRecordIndex")?, "recordIndex", "selector")?;
    trace.check(cell(row, "componentOrdinal")?, original, "conversion")?;
    trace.part(cell(row, "sourceType")?, original, "$type", "selector")?;
    trace.part(cell(row, "configuration")?, original, "$type", "selector")
}

pub(super) fn require_state(cell: &Value, expected: &str) -> TestResult {
    assert!(
        proof::text(cell, "/state")? == expected,
        "the carrier keeps the original read state"
    );
    Ok(())
}

pub(super) fn cell<'row>(row: &'row Value, name: &str) -> TestResult<&'row Value> {
    row.get(name)
        .ok_or_else(|| "expected the authored field carrier".into())
}

pub(super) fn owned(
    source: &StructValue<'_>,
    row: &Value,
    field: &str,
    compare: impl FnOnce(&StructValue<'_>, &Value) -> TestResult,
) -> TestResult {
    let cell = cell(row, field)?;
    match source.field(field) {
        None => require_state(cell, "absent"),
        Some(Original::Null) => require_state(cell, "null"),
        Some(Original::Struct(original)) => {
            let value = cell.get("value").ok_or("original owned source value")?;
            if proof::text(value, "/sourceType/value")? != original.type_name() {
                return Err("original owned source type differs".into());
            }
            compare(original, value)
        }
        _ => Err("unreadable original owned structure".into()),
    }
}

pub(super) fn single(source: &StructValue<'_>, field: &str, cell: &Value) -> TestResult {
    match source.field(field) {
        None => require_state(cell, "absent"),
        Some(Original::Single(expected)) => {
            if proof::text(cell, "/value/rawBits")? != format!("{:08x}", expected.to_bits()) {
                return Err(format!("original Single {field} differs").into());
            }
            require_state(cell, "present")
        }
        _ => Err("unreadable original Single".into()),
    }
}
