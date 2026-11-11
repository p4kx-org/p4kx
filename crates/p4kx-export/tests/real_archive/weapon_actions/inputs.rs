// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::TestResult;
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
    data: &Value,
    kind: &str,
) -> TestResult<usize> {
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
        "beam" => fields(trace, original, data, BEAM)?,
        "charged" => fields(trace, original, data, CHARGED)? + modifier(trace, original, data)?,
        "sequence" => fields(trace, original, data, &["mode"])?,
        "authored" => authored_roots(trace, original, data)?,
        _ => 0,
    };
    Ok(count)
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
    Ok(1 + match expected {
        "projectile" => fields(trace, launcher, data, PROJECTILE)?,
        "missile" => fields(trace, launcher, data, MISSILE)?,
        "throwable" => fields(trace, launcher, data, &["throwMode", "soundRadius"])?,
        "authored" => authored_roots(
            trace,
            launcher,
            data.get("original")
                .ok_or("unlisted launcher source arena")?,
        )?,
        _ => 0,
    })
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
    Ok(count + authored_roots(trace, modifier, arena)?)
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

fn authored_roots<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    arena: &Value,
) -> TestResult<usize> {
    let roots = proof::rows(arena, "/roots")?;
    assert!(
        roots.len() == original.fields().len(),
        "complete original authored field roots"
    );
    for (source, projected) in original.fields().iter().zip(roots) {
        assert!(
            proof::text(projected, "/name/value")? == source.name(),
            "original authored field order"
        );
        trace.part(
            cell(projected, "name")?,
            source.value(),
            "$fieldName",
            "payload",
        )?;
    }
    Ok(roots.len())
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
