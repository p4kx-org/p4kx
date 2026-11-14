// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::{TestResult, inventory};
use crate::original_item_arena;
use crate::real_archive_emp::compare_link;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

const SHOT: &[&str] = &["fireRate", "heatPerShot", "wearPerShot"];
const RAPID: &[&str] = &[
    "fireDuringSpinUp",
    "spinUpTime",
    "spinDownTime",
    "spinParam",
    "audioLoopTimeBetweenFirstShots",
];
const BURST: &[&str] = &["shotCount", "cooldownTime"];
const BEAM: &[&str] = &[
    "hitType",
    "fullDamageRange",
    "zeroDamageRange",
    "heatCostPerSecond",
    "heatPerSecond",
    "wearPerSecond",
    "minEnergyDraw",
    "maxEnergyDraw",
    "chargeUpTime",
    "chargeDownTime",
    "fireDuringChargeUp",
];
const CHARGED: &[&str] = &[
    "chargeTime",
    "overchargeTime",
    "overchargedTime",
    "cooldownTime",
    "maxGlow",
    "interpolateChargeBonus",
    "fireAutomaticallyOnFullCharge",
    "fireOnlyOnFullCharge",
    "chargeAutomatically",
];
const PROJECTILE: &[&str] = &[
    "fireHelper",
    "muzzleHelper",
    "ammoCost",
    "heatCost",
    "pelletCount",
    "damageMultiplier",
    "soundRadius",
    "projectileType",
];
const MISSILE: &[&str] = &[
    "soundRadius",
    "detachVelocityRight",
    "detachVelocityForward",
    "detachVelocityUp",
    "detachVelocityRoll",
    "canFireWithoutLock",
    "showHitIndicator",
    "igniteOnPylon",
];

pub(super) fn action<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    node: &Value,
    data: &Value,
    kind: &str,
) -> TestResult<usize> {
    let recoil_count = recoil(trace, original, node)?;
    let count = match kind {
        "single" | "rapid" | "burst" => {
            let mut count = fields(trace, original, data, SHOT)? + launcher(trace, original, data)?;
            count += match kind {
                "rapid" => fields(trace, original, data, RAPID)?,
                "burst" => fields(trace, original, data, BURST)?,
                _ => 0,
            };
            count
        }
        "beam" => fields(trace, original, data, BEAM)? + beam_damage(trace, original, data)?,
        "aim" => {
            fields(trace, original, data, &["hasReloadModesOnUI"])?
                + owned_input(trace, original, data, "aimModifier")?
                + aim_members(trace, original, data)?
        }
        "charged" => fields(trace, original, data, CHARGED)? + modifier(trace, original, data)?,
        "sequence" => fields(trace, original, data, &["mode"])?,
        _ => 0,
    };
    let arena = if kind == "authored" {
        data
    } else {
        cell(data, "authored")?
    };
    let residual_count = inventory::remaining(trace, original, arena, |name| {
        if matches!(name, "recoil" | "recoilGrappling" | "recoilInterval") {
            node.get(name)
        } else {
            data.get(name)
        }
    })?;
    Ok(count + recoil_count + residual_count)
}

fn recoil<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    node: &Value,
) -> TestResult<usize> {
    let mut count = fields(trace, original, node, &["recoilInterval"])?;
    for name in ["recoil", "recoilGrappling"] {
        let original = original.field(name);
        let selected = cell(node, name)?;
        compare_link(selected, original, trace, false)?;
        count += usize::from(original.is_some());
    }
    Ok(count)
}

fn beam_damage<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    data: &Value,
) -> TestResult<usize> {
    let name = "damagePerSecond";
    let count = owned_input(trace, original, data, name)?;
    let Some(raw @ Original::Struct(damage)) = original.field(name) else {
        return Ok(count);
    };
    let selected = cell(data, name)?;
    trace.part(selected, raw, "$target", "selector")?;
    let projected = selected
        .pointer("/value")
        .ok_or("original beam damage body")?;
    assert!(
        proof::text(projected, "/sourceType/value")? == damage.type_name(),
        "the original nominal DamageBase pointer retains its concrete body"
    );
    let alternative = proof::text(projected, "/type")?;
    let fields: &[(&str, &str)] = match alternative {
        "channels" => &[
            ("DamagePhysical", "physical"),
            ("DamageEnergy", "energy"),
            ("DamageDistortion", "distortion"),
            ("DamageThermal", "thermal"),
            ("DamageBiochemical", "biochemical"),
            ("DamageStun", "stun"),
        ],
        "parameters" => &[("damageTotal", "damageTotal")],
        "base" | "authored" => &[],
        _ => return Err("unrecognized original beam damage alternative".into()),
    };
    let cells = projected.get("value").unwrap_or(&Value::Null);
    let mut checked = count
        + inventory::remaining(trace, damage, cell(projected, "authored")?, |name| {
            fields
                .iter()
                .find(|(original, _)| *original == name)
                .and_then(|(_, wire)| cells.get(wire))
                .or_else(|| cells.get(name))
        })?;
    if alternative == "parameters" {
        checked += inventory::damage_macro(
            trace,
            damage.field("damageMacro"),
            cell(cells, "damageMacro")?,
        )?;
    }
    for &(name, wire) in fields {
        let raw = damage.field(name);
        let output = cell(cells, wire)?;
        if let Some(raw @ Original::Single(number)) = raw {
            assert!(
                u32::from_str_radix(proof::text(output, "/value/rawBits")?, 16)?
                    == number.to_bits(),
                "exact authored beam damage Single"
            );
            trace.check(output, raw, "payload")?;
            checked += 1;
        } else {
            other_state(trace, raw, output)?;
        }
    }
    Ok(checked)
}

fn launcher<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    data: &Value,
) -> TestResult<usize> {
    let raw = original.field("launchParams");
    let selected = cell(data, "launchParams")?;
    let Some(value @ Original::Struct(launcher)) = raw else {
        other_state(trace, raw, selected)?;
        return Ok(0);
    };
    require_state(selected, "present")?;
    trace.part(selected, value, "$type", "selector")?;
    let data = selected.pointer("/value").ok_or("original launcher body")?;
    let expected = match launcher.type_name() {
        "SLauncherBase" => "base",
        "SDummyLauncher" => "dummy",
        "SProjectileLauncher" => "projectile",
        "SMissileLauncher" => "missile",
        "SThrowableLauncher" => "throwable",
        _ => "authored",
    };
    assert!(
        proof::text(data, "/type")? == expected,
        "original launcher kind"
    );
    let (typed, arena) = match expected {
        "projectile" => (PROJECTILE, "authored"),
        "missile" => (MISSILE, "authored"),
        "throwable" => (&["throwMode", "soundRadius"][..], "authored"),
        "authored" => (&[][..], "original"),
        _ => (&[][..], "authored"),
    };
    Ok(1 + fields(trace, launcher, data, typed)?
        + inventory::remaining(trace, launcher, cell(data, arena)?, |name| data.get(name))?
        + launcher_spread(trace, launcher, data)?)
}

fn aim_members<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    data: &Value,
) -> TestResult<usize> {
    let mut checked = unit_source(trace, original, data, "aimGeometryTags")?;
    if let Some(Original::Struct(modifier)) = original.field("aimModifier")
        && let Some(value) = data
            .pointer("/aimModifier/value")
            .filter(|value| value.is_object())
    {
        checked += unit_source(trace, modifier, value, "weaponStats")?;
        checked +=
            inventory::remaining(trace, modifier, cell(value, "additionalFields")?, |name| {
                value.get(name)
            })?;
    }
    Ok(checked)
}

fn unit_source<'source>(
    trace: &Trace<'_, 'source>,
    source: &StructValue<'source>,
    data: &Value,
    name: &str,
) -> TestResult<usize> {
    let count = owned_input(trace, source, data, name)?;
    let Some(Original::Struct(source)) = source.field(name) else {
        return Ok(count);
    };
    let value = cell(cell(data, name)?, "value")?;
    Ok(count
        + original_item_arena::compare(
            trace,
            source.fields().iter(),
            cell(value, "additionalFields")?,
        )?
        .nodes)
}

fn launcher_spread<'source>(
    trace: &Trace<'_, 'source>,
    launcher: &StructValue<'source>,
    data: &Value,
) -> TestResult<usize> {
    if let Some(Original::Struct(spread)) = launcher.field("spreadParams")
        && let Some(value) = data
            .pointer("/spreadParams/value")
            .filter(|value| value.is_object())
    {
        let count = fields(
            trace,
            spread,
            value,
            &["min", "max", "firstAttack", "attack", "decay"],
        )?;
        return Ok(count + inventory::authored(trace, spread, value)?);
    }
    Ok(0)
}

fn modifier<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    data: &Value,
) -> TestResult<usize> {
    let name = "maxChargeModifier";
    let count = owned_input(trace, original, data, name)?;
    let Some(Original::Struct(modifier)) = original.field(name) else {
        return Ok(count);
    };
    let arena = cell(data, name)?
        .pointer("/value")
        .ok_or("original charge modifier arena")?;
    Ok(count + original_item_arena::compare(trace, modifier.fields().iter(), arena)?.nodes)
}

pub(super) fn fields<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    data: &Value,
    fields: &[&str],
) -> TestResult<usize> {
    let mut checked = 0;
    for &name in fields {
        let raw = original.field(name);
        let selected = cell(data, name)?;
        let value = selected
            .get("value")
            .ok_or("original input value carrier")?;
        let equal = match raw {
            Some(Original::Single(source)) => {
                u32::from_str_radix(proof::text(value, "/rawBits")?, 16)? == source.to_bits()
            }
            Some(Original::Bool(source)) => value.as_bool() == Some(*source),
            Some(Original::Int32(source)) => value.as_i64() == Some(i64::from(*source)),
            Some(Original::UInt8(source)) => value.as_u64() == Some(u64::from(*source)),
            Some(Original::Enum(source) | Original::String(source)) => {
                value.as_str() == Some(*source)
            }
            _ => {
                other_state(trace, raw, selected)?;
                continue;
            }
        };
        let raw = raw.ok_or("the original input is present")?;
        assert!(equal, "{name} preserves the exact stored input");
        require_state(selected, "present")?;
        trace.check(selected, raw, "payload")?;
        checked += 1;
    }
    Ok(checked)
}

pub(super) fn owned_input<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    data: &Value,
    name: &str,
) -> TestResult<usize> {
    let raw = original.field(name);
    let selected = cell(data, name)?;
    let Some(value @ Original::Struct(_)) = raw else {
        other_state(trace, raw, selected)?;
        return Ok(0);
    };
    require_state(selected, "present")?;
    trace.part(selected, value, "$type", "selector")?;
    Ok(1)
}

pub(super) fn other_state<'source>(
    trace: &Trace<'_, 'source>,
    original: Option<&Original<'source>>,
    cell: &Value,
) -> TestResult {
    match original {
        None => require_state(cell, "absent"),
        Some(raw @ Original::Null) => {
            require_state(cell, "null")?;
            trace.check(cell, raw, "selector")
        }
        Some(raw @ Original::Error(_)) => {
            require_state(cell, "unreadable")?;
            trace.attempted_value(cell, raw)
        }
        Some(_) => Err("unexpected original storage for selected typed input".into()),
    }
}
