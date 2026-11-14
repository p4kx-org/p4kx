// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{DataForge, DataType, Field, StructValue, Value as Original};
use serde_json::Value;

use super::{Counts, TestResult};
use crate::original_item_arena as arena;
use crate::original_item_numbers as numbers;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::Trace;

const ROOT: &[(&str, &str)] = &[
    ("quantumFuelRequirement", "fuelRequirement"),
    ("jumpRange", "jumpRange"),
    ("disconnectRange", "disconnectRange"),
];
const PROFILE: &[(&str, &str)] = &[
    ("driveSpeed", "driveSpeed"),
    ("cooldownTime", "cooldownTime"),
    ("spoolUpTime", "spoolUpTime"),
    ("stageOneAccelRate", "stageOneAccelRate"),
    ("stageTwoAccelRate", "stageTwoAccelRate"),
    ("engageSpeed", "engageSpeed"),
    ("calibrationRate", "calibrationRate"),
    ("minCalibrationRequirement", "minCalibrationRequirement"),
    ("maxCalibrationRequirement", "maxCalibrationRequirement"),
    (
        "calibrationProcessAngleLimit",
        "calibrationProcessAngleLimit",
    ),
    (
        "calibrationWarningAngleLimit",
        "calibrationWarningAngleLimit",
    ),
    ("calibrationDelayInSeconds", "calibrationDelayInSeconds"),
    ("interdictionEffectTime", "interdictionEffectTime"),
    ("VFXSpoolEndVelocity", "vfxSpoolEndVelocity"),
    ("VFXPinchEffectTime", "vfxPinchEffectTime"),
    ("VFXPinchMaxVelocity", "vfxPinchMaxVelocity"),
    ("VFXEntryFlashVelocity", "vfxEntryFlashVelocity"),
    ("VFXTrailStartVelocity", "vfxTrailStartVelocity"),
    (
        "VFXTravelEffectStartVelocity",
        "vfxTravelEffectStartVelocity",
    ),
    ("VFXTravelEffectEndVelocity", "vfxTravelEffectEndVelocity"),
    ("VFXExitEffectVelocity", "vfxExitEffectVelocity"),
    ("ShaderNodeEngageVelocity", "shaderNodeEngageVelocity"),
    ("ShaderNodeShutdownVelocity", "shaderNodeShutdownVelocity"),
    (
        "ShaderNodeMaxStrengthVelocity",
        "shaderNodeMaxStrengthVelocity",
    ),
];
const HEAT: &[(&str, &str)] = &[
    ("preRampUpThermalEnergyDraw", "preRampUpThermalEnergyDraw"),
    ("rampUpThermalEnergyDraw", "rampUpThermalEnergyDraw"),
    ("inFlightThermalEnergyDraw", "inFlightThermalEnergyDraw"),
    ("rampDownThermalEnergyDraw", "rampDownThermalEnergyDraw"),
    (
        "postRampDownThermalEnergyDraw",
        "postRampDownThermalEnergyDraw",
    ),
];
const BOOST: &[(&str, &str)] = &[
    ("maxBoostSpeed", "maxBoostSpeed"),
    ("timeToMaxBoostSpeed", "timeToMaxBoostSpeed"),
    ("boostUseTime", "boostUseTime"),
    ("boostRechargeTime", "boostRechargeTime"),
    ("stopTime", "stopTime"),
    ("minJumpDistance", "minJumpDistance"),
    ("ifcsHandoverDownTime", "ifcsHandoverDownTime"),
    ("ifcsHandoverRespoolTime", "ifcsHandoverRespoolTime"),
];
const MEMBERS: &[&str] = &["idleState", "travellingState", "effectTags"];

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum State {
    Present,
    Absent,
    Null,
    Unreadable,
    Invalid,
    Unsupported,
}

impl State {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Absent => "absent",
            Self::Null => "null",
            Self::Unreadable => "unreadable",
            Self::Invalid => "invalid",
            Self::Unsupported => "unsupported",
        }
    }

    fn check(self, exported: &Value) -> TestResult {
        require_state(exported, self.label())?;
        if self != Self::Present {
            assert!(
                cell(exported, "value")?.is_null(),
                "non-present cells have null payload"
            );
        }
        Ok(())
    }

    const fn consumes(self) -> bool {
        matches!(self, Self::Present | Self::Null)
    }
}

pub(super) fn witnesses_modes_and_heat(source: &StructValue<'_>) -> TestResult<bool> {
    let speed = |name| {
        let Some(Original::Struct(profile)) = source.field(name) else {
            return Ok(None);
        };
        profile
            .field("driveSpeed")
            .map_or(Ok(None), numbers::expected)
    };
    let (Some(standard), Some(spline)) = (speed("params")?, speed("splineJumpParams")?) else {
        return Ok(false);
    };
    let Some(Original::Struct(heat)) = source.field("heatParams") else {
        return Ok(false);
    };
    for &(name, _) in HEAT {
        let Some(raw) = heat.field(name) else {
            return Ok(false);
        };
        if numbers::expected(raw)?.is_none() {
            return Ok(false);
        }
    }
    Ok(standard != spline)
}

pub(super) fn compare<'source>(
    forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    source: &StructValue<'source>,
    exported: &Value,
    counts: &mut Counts,
) -> TestResult {
    arena::source_type(trace, raw, source, exported)?;
    let mut consumed = compare_numbers(trace, raw, source, exported, ROOT, counts)?;
    for (name, key, mappings, members) in [
        ("params", "standard", PROFILE, MEMBERS),
        ("splineJumpParams", "spline", PROFILE, MEMBERS),
        ("heatParams", "heat", HEAT, &[][..]),
        ("quantumBoostParams", "boost", BOOST, &[][..]),
    ] {
        let cell = cell(exported, key)?;
        let field = find(source, name);
        let state = compare_class(forge, trace, raw, field, cell)?;
        if let (State::Present, Some(field)) = (state, field) {
            compare_block(trace, field.value(), cell, mappings, members, counts)?;
        }
        if state.consumes() {
            consumed.insert(name);
        }
        *counts.blocks.entry((key, state)).or_default() += 1;
    }
    arena::additional(trace, source, exported, consumed)?;
    Ok(())
}

fn compare_block<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    projected: &Value,
    mappings: &[(&'static str, &str)],
    members: &[&'static str],
    counts: &mut Counts,
) -> TestResult {
    let Original::Struct(source) = raw else {
        return Err("original quantum Class block".into());
    };
    let output = cell(projected, "value")?;
    arena::source_type(trace, raw, source, output)?;
    let mut consumed = compare_numbers(trace, raw, source, output, mappings, counts)?;
    for &name in members {
        if compare_member(trace, raw, find(source, name), cell(output, name)?)?.consumes() {
            consumed.insert(name);
        }
    }
    arena::additional(trace, source, output, consumed)?;
    Ok(())
}

fn compare_numbers<'source>(
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    source: &StructValue<'source>,
    output: &Value,
    mappings: &[(&'static str, &str)],
    counts: &mut Counts,
) -> TestResult<BTreeSet<&'static str>> {
    let mut consumed = BTreeSet::new();
    for &(name, key) in mappings {
        let exported = cell(output, key)?;
        let Some(raw) = original(trace, owner, find(source, name), exported)? else {
            continue;
        };
        let state = if let Some(expected) = numbers::expected(raw)? {
            State::Present.check(exported)?;
            assert!(
                numbers::equal(&expected, cell(exported, "value")?),
                "exact original numeric payload"
            );
            trace.check(exported, raw, "payload")?;
            counts.numbers += 1;
            State::Present
        } else {
            non_present(trace, raw, exported, State::Null)?
        };
        if state.consumes() {
            consumed.insert(name);
        }
    }
    Ok(consumed)
}

fn compare_member<'source>(
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    field: Option<&Field<'source>>,
    exported: &Value,
) -> TestResult<State> {
    let Some(raw) = original(trace, owner, field, exported)? else {
        return Ok(State::Absent);
    };
    if matches!(raw, Original::Null | Original::Error(_)) {
        return non_present(trace, raw, exported, State::Null);
    }
    State::Present.check(exported)?;
    trace.check(exported, raw, "selector")?;
    arena::compare(trace, field.into_iter(), cell(exported, "value")?)?;
    Ok(State::Present)
}

fn compare_class<'source>(
    forge: &DataForge<'_>,
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    field: Option<&Field<'source>>,
    exported: &Value,
) -> TestResult<State> {
    let Some(raw) = original(trace, owner, field, exported)? else {
        return Ok(State::Absent);
    };
    let storage = field.ok_or("original quantum Class declaration")?.storage();
    trace.part(exported, raw, "$dataType", "selector")?;
    trace.part(exported, raw, "$isArray", "selector")?;
    if storage.data_type() != DataType::Class || storage.is_array() {
        return non_present(trace, raw, exported, State::Invalid);
    }
    trace.part(exported, raw, "$target", "selector")?;
    let Some(declared) = forge
        .struct_at(u32::from(storage.target()))
        .and_then(|structure| structure.name().ok())
    else {
        State::Unreadable.check(exported)?;
        trace.attempted_struct_name(exported, storage.target())?;
        return Ok(State::Unreadable);
    };
    trace.struct_name(exported, storage.target())?;
    let Original::Struct(structure) = raw else {
        return non_present(trace, raw, exported, State::Null);
    };
    let state = if structure.type_name() == declared {
        State::Present
    } else {
        State::Unsupported
    };
    state.check(exported)?;
    trace.part(exported, raw, "$type", "selector")?;
    Ok(state)
}

fn original<'read, 'source>(
    trace: &Trace<'_, 'source>,
    owner: &Original<'source>,
    field: Option<&'read Field<'source>>,
    exported: &Value,
) -> TestResult<Option<&'read Original<'source>>> {
    if let Some(field) = field {
        let raw = field.value();
        trace.part(exported, raw, "$fieldName", "selector")?;
        return Ok(Some(raw));
    }
    State::Absent.check(exported)?;
    trace.part(exported, owner, "$type", "selector")?;
    trace.part(exported, owner, "$fields", "selector")?;
    Ok(None)
}

fn non_present<'source>(
    trace: &Trace<'_, 'source>,
    raw: &Original<'source>,
    exported: &Value,
    null: State,
) -> TestResult<State> {
    if matches!(raw, Original::Error(_)) {
        State::Unreadable.check(exported)?;
        trace.attempted_value(exported, raw)?;
        return Ok(State::Unreadable);
    }
    let state = if matches!(raw, Original::Null) {
        null
    } else {
        State::Invalid
    };
    state.check(exported)?;
    trace.check(exported, raw, "selector")?;
    Ok(state)
}

fn find<'read, 'source>(
    source: &'read StructValue<'source>,
    name: &str,
) -> Option<&'read Field<'source>> {
    source.fields().iter().find(|field| field.name() == name)
}
