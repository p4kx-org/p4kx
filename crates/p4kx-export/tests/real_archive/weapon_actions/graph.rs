// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, VecDeque};

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::{Coverage, TestResult, inputs};
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

struct Pending<'tree, 'source> {
    original: &'tree Original<'source>,
    index: usize,
}

pub(super) fn compare<'source>(
    trace: &Trace<'_, 'source>,
    weapon: &StructValue<'source>,
    row: &Value,
    coverage: &mut BTreeMap<String, Coverage>,
) -> TestResult {
    let nodes = proof::rows(row, "/actionNodes")?;
    let mut pending = VecDeque::new();
    slots(trace, weapon, row, "fireActions", &mut pending)?;
    slot(
        trace,
        weapon.field("aimAction"),
        cell(row, "aimAction")?,
        &mut pending,
    )?;
    let mut processed = 0;
    while let Some(next) = pending.pop_front() {
        assert!(
            next.index == processed,
            "action indices follow original discovery order"
        );
        let node = nodes.get(next.index).ok_or("original action node index")?;
        let Original::Struct(original) = next.original else {
            return Err("present original action is a Struct".into());
        };
        assert!(
            proof::text(node, "/sourceType/value")? == original.type_name(),
            "action node preserves its concrete declaration"
        );
        assert!(
            proof::text(node, "/sourceValueOrdinal/value")?
                == trace.ordinal(next.original)?.to_string(),
            "action node uses its exact original occurrence ordinal"
        );
        trace.part(
            cell(node, "sourceType")?,
            next.original,
            "$type",
            "selector",
        )?;
        trace.check(
            cell(node, "sourceValueOrdinal")?,
            next.original,
            "conversion",
        )?;
        let kind = kind(original.type_name());
        assert!(
            proof::text(node, "/kind/type")? == kind,
            "original action kind"
        );
        let data = node.pointer("/kind/value").ok_or("action kind payload")?;
        let measured = coverage.entry(original.type_name().to_owned()).or_default();
        measured.nodes += 1;
        measured.inputs += inputs::action(trace, original, data, kind)?;
        measured.inputs += children(trace, original, data, kind, &mut pending)?;
        processed += 1;
    }
    assert!(
        processed == nodes.len(),
        "every exported node matches an original action slot"
    );
    Ok(())
}

fn children<'tree, 'source>(
    trace: &Trace<'_, 'source>,
    original: &'tree StructValue<'source>,
    data: &Value,
    kind: &str,
    pending: &mut VecDeque<Pending<'tree, 'source>>,
) -> TestResult<usize> {
    match kind {
        "sequence" => entries(trace, original, data, Entries::Sequence, pending),
        "parallel" => {
            slots(trace, original, data, "weaponActions", pending)?;
            Ok(0)
        }
        "conditional" | "dynamicConditional" => {
            let count = entries(trace, original, data, Entries::Conditional, pending)?;
            slot(
                trace,
                original.field("defaultWeaponAction"),
                cell(data, "defaultWeaponAction")?,
                pending,
            )?;
            Ok(count)
        }
        "toggle" | "charged" => {
            slot(
                trace,
                original.field("weaponAction"),
                cell(data, "weaponAction")?,
                pending,
            )?;
            Ok(0)
        }
        _ => Ok(0),
    }
}

fn kind(source_type: &str) -> &'static str {
    match source_type {
        "SWeaponActionFireSingleParams" => "single",
        "SWeaponActionFireRapidParams" => "rapid",
        "SWeaponActionFireBurstParams" => "burst",
        "SWeaponActionFireBeamParams" => "beam",
        "SWeaponActionAimSimpleParams" => "aim",
        "SWeaponActionSequenceParams" => "sequence",
        "SWeaponActionParallelParams" => "parallel",
        "SWeaponActionConditionParams" => "conditional",
        "SWeaponActionDynamicConditionParams" => "dynamicConditional",
        "SWeaponActionToggleParams" => "toggle",
        "SWeaponActionFireChargedParams" => "charged",
        _ => "authored",
    }
}

fn slots<'tree, 'source>(
    trace: &Trace<'_, 'source>,
    owner: &'tree StructValue<'source>,
    row: &Value,
    name: &str,
    pending: &mut VecDeque<Pending<'tree, 'source>>,
) -> TestResult {
    let Some((original, projected)) = array(trace, owner, row, name)? else {
        return Ok(());
    };
    for (value, cell) in original.iter().zip(projected) {
        slot(trace, Some(value), cell, pending)?;
    }
    Ok(())
}

fn slot<'tree, 'source>(
    trace: &Trace<'_, 'source>,
    original: Option<&'tree Original<'source>>,
    cell: &Value,
    pending: &mut VecDeque<Pending<'tree, 'source>>,
) -> TestResult {
    if let Some(raw @ Original::Struct(_)) = original {
        require_state(cell, "present")?;
        trace.part(cell, raw, "$type", "selector")?;
        let index = usize::try_from(proof::number(cell, "/value")?)?;
        pending.push_back(Pending {
            original: raw,
            index,
        });
        return Ok(());
    }
    inputs::other_state(trace, original, cell)
}

#[derive(Clone, Copy)]
enum Entries {
    Sequence,
    Conditional,
}

impl Entries {
    const fn field(self) -> &'static str {
        match self {
            Self::Sequence => "sequenceEntries",
            Self::Conditional => "conditionalWeaponActions",
        }
    }
}

fn entries<'tree, 'source>(
    trace: &Trace<'_, 'source>,
    owner: &'tree StructValue<'source>,
    row: &Value,
    shape: Entries,
    pending: &mut VecDeque<Pending<'tree, 'source>>,
) -> TestResult<usize> {
    let Some((original, projected)) = array(trace, owner, row, shape.field())? else {
        return Ok(0);
    };
    let mut count = 0;
    for (raw, cell) in original.iter().zip(projected) {
        let Original::Struct(entry) = raw else {
            inputs::other_state(trace, Some(raw), cell)?;
            continue;
        };
        require_state(cell, "present")?;
        trace.part(cell, raw, "$type", "selector")?;
        let data = cell
            .pointer("/value")
            .ok_or("original action entry payload")?;
        count += match shape {
            Entries::Sequence => {
                inputs::fields(trace, entry, data, &["delay", "unit", "repetitions"])?
            }
            Entries::Conditional => inputs::owned_input(trace, entry, data, "condition")?,
        };
        slot(
            trace,
            entry.field("weaponAction"),
            super::cell(data, "weaponAction")?,
            pending,
        )?;
    }
    Ok(count)
}

fn array<'tree, 'source, 'catalog>(
    trace: &Trace<'_, 'source>,
    owner: &'tree StructValue<'source>,
    row: &'catalog Value,
    name: &str,
) -> TestResult<Option<(&'tree [Original<'source>], &'catalog [Value])>> {
    let original = owner.field(name);
    let cell = cell(row, name)?;
    let Some(raw @ Original::Array(values)) = original else {
        inputs::other_state(trace, original, cell)?;
        return Ok(None);
    };
    require_state(cell, "present")?;
    trace.check(cell, raw, "selector")?;
    let projected = proof::rows(cell, "/value")?;
    assert!(
        values.len() == projected.len(),
        "original array slot count for {name}"
    );
    Ok(Some((values, projected)))
}
