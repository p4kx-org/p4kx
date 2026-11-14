// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, DataType, Field, StructValue, Value as Original};
use serde_json::Value;

use super::{Counts, TestResult};
use crate::original_item_arena::{self as arena, source_type};
use crate::original_item_numbers as numbers;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification::print_line;

type Mapping = (&'static str, &'static str);

const SHIELD: [Mapping; 13] = [
    ("MaxShieldHealth", "maxHealth"),
    ("MaxShieldRegen", "maxRegen"),
    ("DecayRatio", "decayRatio"),
    ("DownedRegenDelay", "downedRegenDelay"),
    ("DamagedRegenDelay", "damagedRegenDelay"),
    (
        "ReservePoolInitialHealthRatio",
        "reservePoolInitialHealthRatio",
    ),
    ("ReservePoolMaxHealthRatio", "reservePoolMaxHealthRatio"),
    ("ReservePoolRegenRateRatio", "reservePoolRegenRateRatio"),
    ("ReservePoolDrainRateRatio", "reservePoolDrainRateRatio"),
    (
        "ElectricalChargeDamageResistance",
        "electricalChargeDamageResistance",
    ),
    ("stunParams", "stunParams"),
    ("ShieldResistance", "shieldResistance"),
    ("ShieldAbsorption", "shieldAbsorption"),
];
const STUN: [Mapping; 4] = [
    ("minAlphaDamageRatio", "minAlphaDamageRatio"),
    ("maxAlphaDamageRatio", "maxAlphaDamageRatio"),
    ("minStunTime", "minStunTime"),
    ("maxStunTime", "maxStunTime"),
];
const RANGE: [Mapping; 2] = [("Min", "min"), ("Max", "max")];
const SCALAR_COUNT: usize = SHIELD.len() - 3;

#[derive(Default)]
pub(super) struct ShieldFields([Coverage; SHIELD.len()]);
#[derive(Default)]
pub(super) struct StunFields([Coverage; STUN.len()]);
#[derive(Default)]
pub(super) struct RangeFields([Coverage; RANGE.len()]);

#[derive(Default)]
struct Coverage {
    present: usize,
    absent: usize,
    null: usize,
    invalid: usize,
    unreadable: usize,
    unsupported: usize,
}

impl Coverage {
    fn observe(&mut self, exported: &Value) -> TestResult<bool> {
        let state = proof::text(exported, "/state")?;
        match state {
            "present" => self.present += 1,
            "absent" => self.absent += 1,
            "null" => self.null += 1,
            "invalid" => self.invalid += 1,
            "unreadable" => self.unreadable += 1,
            "unsupported" => self.unsupported += 1,
            _ => return Err("unexpected shield cell state".into()),
        }
        Ok(matches!(state, "present" | "null"))
    }
}

impl ShieldFields {
    pub(super) fn print(&self, group: &str) {
        print(group, &SHIELD, &self.0);
    }
}

impl StunFields {
    pub(super) fn print(&self, group: &str) {
        print(group, &STUN, &self.0);
    }
}

impl RangeFields {
    pub(super) fn print(&self, group: &str) {
        print(group, &RANGE, &self.0);
    }
}

fn print(group: &str, mappings: &[Mapping], counts: &[Coverage]) {
    for ((_, key), count) in mappings.iter().zip(counts) {
        print_line(&format!(
            "{group}.{key}: {} present, {} absent, {} null, {} invalid, {} unreadable, {} unsupported",
            count.present,
            count.absent,
            count.null,
            count.invalid,
            count.unreadable,
            count.unsupported,
        ));
    }
}

pub(super) fn compare<'source>(
    forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    source: &StructValue<'source>,
    exported: &Value,
    counts: &mut Counts,
) -> TestResult {
    source_type(trace, raw, source, exported)?;
    let (scalar_mappings, classes) = SHIELD.split_at(SCALAR_COUNT);
    let (scalar_counts, _) = counts.shield_fields.0.split_at_mut(SCALAR_COUNT);
    let mut consumed = fields(trace, raw, source, exported, scalar_mappings, scalar_counts)?;
    for (offset, &(name, key)) in classes.iter().enumerate() {
        let projected = cell(exported, key)?;
        let field = find(source, name);
        if let Some((value, body, declaration)) =
            class(forge, trace, raw, field, projected, key != "stunParams")?
        {
            match key {
                "stunParams" => stun(trace, value, body, counts)?,
                "shieldResistance" => ranges(
                    trace,
                    value,
                    body,
                    declaration,
                    RangeGroup::Resistance,
                    counts,
                )?,
                "shieldAbsorption" => ranges(
                    trace,
                    value,
                    body,
                    declaration,
                    RangeGroup::Absorption,
                    counts,
                )?,
                _ => return Err("unmapped shield Class field".into()),
            }
        }
        let coverage = counts
            .shield_fields
            .0
            .get_mut(SCALAR_COUNT + offset)
            .ok_or("shield Class coverage")?;
        if coverage.observe(projected)? {
            consumed.insert(name);
        }
    }
    count_additional(trace, source, exported, consumed, counts)
}

fn stun<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    exported: &Value,
    counts: &mut Counts,
) -> TestResult {
    let Original::Struct(source) = raw else {
        return Err("original stun Class must be a struct".into());
    };
    source_type(trace, raw, source, exported)?;
    let consumed = fields(
        trace,
        raw,
        source,
        exported,
        &STUN,
        &mut counts.stun_fields.0,
    )?;
    count_additional(trace, source, exported, consumed, counts)
}

#[derive(Clone, Copy)]
enum RangeGroup {
    Resistance,
    Absorption,
}

fn ranges<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    exported: &Value,
    declaration: &str,
    group: RangeGroup,
    counts: &mut Counts,
) -> TestResult {
    let Original::Array(source) = raw else {
        return Err("original range Class array must be an array".into());
    };
    let projected = exported.as_array().ok_or("projected range array")?;
    assert!(
        projected.len() == source.len(),
        "every physical shield range slot is retained"
    );
    for (raw, projected) in source.iter().zip(projected) {
        let Original::Struct(structure) = raw else {
            other(trace, Some(raw), projected)?;
            continue;
        };
        trace.part(projected, raw, "$type", "selector")?;
        if structure.type_name() != declaration {
            absent_payload(projected, "unsupported")?;
            continue;
        }
        require_state(projected, "present")?;
        let body = cell(projected, "value")?;
        source_type(trace, raw, structure, body)?;
        let coverage = match group {
            RangeGroup::Resistance => &mut counts.resistance_fields.0,
            RangeGroup::Absorption => &mut counts.absorption_fields.0,
        };
        let consumed = fields(trace, raw, structure, body, &RANGE, coverage)?;
        count_additional(trace, structure, body, consumed, counts)?;
    }
    Ok(())
}

fn class<'catalog, 'read, 'source, 'forge>(
    forge: &'forge DataForge<'_>,
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    field: Option<&'read Field<'source>>,
    exported: &'catalog Value,
    array: bool,
) -> TestResult<Option<(&'read Original<'source>, &'catalog Value, &'forge str)>> {
    let Some(field) = field else {
        other(trace, None, exported)?;
        trace.part(exported, owner, "$type", "selector")?;
        trace.part(exported, owner, "$fields", "selector")?;
        return Ok(None);
    };
    let raw = field.value();
    let storage = field.storage();
    trace.part(exported, raw, "$fieldName", "selector")?;
    trace.part(exported, raw, "$dataType", "selector")?;
    trace.part(exported, raw, "$isArray", "selector")?;
    if storage.data_type() != DataType::Class || storage.is_array() != array {
        if matches!(raw, Original::Error(_)) {
            other(trace, Some(raw), exported)?;
        } else {
            absent_payload(exported, "invalid")?;
            trace.check(exported, raw, "selector")?;
        }
        return Ok(None);
    }
    trace.part(exported, raw, "$target", "selector")?;
    let Some(declaration) = forge
        .struct_at(u32::from(storage.target()))
        .and_then(|target| target.name().ok())
    else {
        absent_payload(exported, "unreadable")?;
        trace.attempted_struct_name(exported, storage.target())?;
        return Ok(None);
    };
    trace.struct_name(exported, storage.target())?;
    match (array, raw) {
        (true, Original::Array(_)) => trace.check(exported, raw, "selector")?,
        (false, Original::Struct(value)) => {
            trace.part(exported, raw, "$type", "selector")?;
            if value.type_name() != declaration {
                absent_payload(exported, "unsupported")?;
                return Ok(None);
            }
        }
        _ => {
            other(trace, Some(raw), exported)?;
            return Ok(None);
        }
    }
    require_state(exported, "present")?;
    Ok(Some((raw, cell(exported, "value")?, declaration)))
}

fn absent_payload(exported: &Value, state: &str) -> TestResult {
    require_state(exported, state)?;
    assert!(
        cell(exported, "value")?.is_null(),
        "non-present cells have no typed payload"
    );
    Ok(())
}

fn fields<'source>(
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    source: &StructValue<'source>,
    exported: &Value,
    mappings: &[Mapping],
    coverage: &mut [Coverage],
) -> TestResult<BTreeSet<&'static str>> {
    let mut consumed = BTreeSet::new();
    for (&(name, key), coverage) in mappings.iter().zip(coverage) {
        let projected = cell(exported, key)?;
        let raw = find(source, name).map(Field::value);
        if let Some(raw) = raw {
            trace.part(projected, raw, "$fieldName", "selector")?;
        } else {
            trace.part(projected, owner, "$type", "selector")?;
            trace.part(projected, owner, "$fields", "selector")?;
        }
        if let Some((raw, expected)) = raw.map_or(Ok(None), |raw| {
            Ok::<_, Box<dyn std::error::Error>>(numbers::expected(raw)?.map(|value| (raw, value)))
        })? {
            require_state(projected, "present")?;
            assert!(
                numbers::equal(&expected, cell(projected, "value")?),
                "the exact original number and width are retained"
            );
            trace.check(projected, raw, "payload")?;
        } else {
            other(trace, raw, projected)?;
        }
        if coverage.observe(projected)? {
            consumed.insert(name);
        }
    }
    Ok(consumed)
}

fn find<'read, 'source>(
    source: &'read StructValue<'source>,
    name: &str,
) -> Option<&'read Field<'source>> {
    source.fields().iter().find(|field| field.name() == name)
}

fn other<'source>(
    trace: &Trace<'_, 'source>,
    raw: Option<&Original<'source>>,
    exported: &Value,
) -> TestResult {
    assert!(
        cell(exported, "value")?.is_null(),
        "non-present cells have no typed payload"
    );
    match raw {
        None => require_state(exported, "absent"),
        Some(raw @ Original::Null) => {
            require_state(exported, "null")?;
            trace.check(exported, raw, "selector")
        }
        Some(raw @ Original::Error(_)) => {
            require_state(exported, "unreadable")?;
            trace.attempted_value(exported, raw)
        }
        Some(raw) => {
            require_state(exported, "invalid")?;
            trace.check(exported, raw, "selector")
        }
    }
}

fn count_additional<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    exported: &Value,
    consumed: BTreeSet<&'static str>,
    counts: &mut Counts,
) -> TestResult {
    let arena = arena::additional(trace, source, exported, consumed)?;
    counts.authored_roots += arena.roots;
    counts.authored_nodes += arena.nodes;
    Ok(())
}
