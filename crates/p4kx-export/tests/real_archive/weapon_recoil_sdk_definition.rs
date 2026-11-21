// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::equipment::WeaponRecoilSnapshot;
use p4kx_extraction::{DataForgeEntry, RecordTree, Value as Raw};
use serde_json::Value;

use crate::original_item_arena;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

use super::{TestResult, UNITS, check_coverage, source_scope};

#[derive(Default)]
pub(super) struct Counts {
    pub(super) units: usize,
    pub(super) nodes: usize,
}

pub(super) fn verify(
    entry: &DataForgeEntry,
    target: &RecordTree<'_>,
    snapshot: &WeaponRecoilSnapshot<'_>,
) -> TestResult<Counts> {
    check_coverage(
        snapshot.coverage(),
        snapshot.requested_languages().is_empty(),
    )?;
    let [row] = snapshot.rows() else {
        return Err("one physically selected recoil definition".into());
    };
    if row.source().record_index().value() != Some(&target.record_index())
        || row.source().guid().value() != Some(&target.guid())
        || row.source().source_type().value() != Some(&target.type_name())
    {
        return Err(
            "the SDK must preserve the independently selected original definition header".into(),
        );
    }
    // Compare wire decimal projections because the in-memory JSON serializer widens f32.
    let rendered = serde_json::from_slice(&serde_json::to_vec(row)?)?;
    let inventory = serde_json::to_value(snapshot.source_inventory())?;
    let uses = proof::indexed(&inventory, "/uses", "id")?;
    let (scope, encrypted) = source_scope(entry)?;
    let trace = Trace::of(target, &uses, &scope, encrypted);
    let header = cell(&rendered, "source")?;
    trace.header(cell(header, "recordIndex")?, "recordIndex", "selector")?;
    trace.header(cell(header, "guid")?, "guid", "payload")?;
    compare_units(&trace, target, &rendered)
}

fn compare_units<'source>(
    trace: &Trace<'_, 'source>,
    target: &RecordTree<'source>,
    row: &Value,
) -> TestResult<Counts> {
    let mut counts = Counts::default();
    let mut consumed = BTreeSet::new();
    for &(raw, rendered, kind) in UNITS {
        let output = cell(row, rendered)?;
        match target.field(raw) {
            Some(value @ Raw::Struct(unit)) if unit.type_name() == kind => {
                require_state(output, "present")?;
                trace.part(output, value, "$type", "selector")?;
                let value_row = cell(output, "value")?;
                original_item_arena::source_type(trace, value, unit, value_row)?;
                counts.nodes += original_item_arena::compare(
                    trace,
                    unit.fields().iter(),
                    cell(value_row, "additionalFields")?,
                )?
                .nodes;
                counts.units += 1;
                consumed.insert(raw);
            }
            Some(value @ Raw::Null) => {
                require_state(output, "null")?;
                trace.check(output, value, "selector")?;
                consumed.insert(raw);
            }
            None => require_state(output, "absent")?,
            Some(_) => {
                return Err(
                    "the selected recoil unit must retain a readable declared shape".into(),
                );
            }
        }
    }
    let remaining = target
        .fields()
        .iter()
        .filter(|field| !consumed.remove(field.name()));
    counts.nodes +=
        original_item_arena::compare(trace, remaining, cell(row, "additionalFields")?)?.nodes;
    if counts.units == 0 || counts.nodes == 0 {
        return Err("at least one original recoil unit and its data must be checked".into());
    }
    Ok(counts)
}
