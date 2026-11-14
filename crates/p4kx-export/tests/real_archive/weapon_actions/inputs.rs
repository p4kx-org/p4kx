// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::TestResult;
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
                + owned_input(trace, original, data, "aimGeometryTags")?
        }
        "charged" => fields(trace, original, data, CHARGED)? + modifier(trace, original, data)?,
        "sequence" => fields(trace, original, data, &["mode"])?,
        "authored" => authored_roots_except_recoil(trace, original, data, node)?,
        _ => 0,
    };
    Ok(count + recoil_count)
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
        "base" => return Ok(count),
        "authored" => {
            return Ok(count + authored_roots(trace, damage, cell(projected, "authored")?)?);
        }
        _ => return Err("unrecognized original beam damage alternative".into()),
    };
    let cells = projected
        .pointer("/value")
        .ok_or("original beam damage inputs")?;
    let mut checked = count;
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
    check_roots(trace, original, arena, |_| true)
}

fn authored_roots_except_recoil<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    arena: &Value,
    node: &Value,
) -> TestResult<usize> {
    check_roots(trace, original, arena, |name| {
        !matches!(name, "recoil" | "recoilGrappling" | "recoilInterval")
            || !matches!(
                node.get(name)
                    .and_then(|value| value.get("state"))
                    .and_then(Value::as_str),
                Some("present" | "null")
            )
    })
}

fn check_roots<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    arena: &Value,
    keep: impl Fn(&str) -> bool,
) -> TestResult<usize> {
    let roots = proof::rows(arena, "/roots")?;
    let mut fields = original.fields().iter().filter(|field| keep(field.name()));
    for projected in roots {
        let source = fields.next().ok_or("unexpected original field root")?;
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
    assert!(
        fields.next().is_none(),
        "complete original authored field roots"
    );
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
