// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value as Original};
use serde_json::Value;

use super::{Counts, TestResult, Trace, cell, compare_fields, proof, require_state};

pub(super) const PRIME_FLAGS: &[(&str, &str)] = &[
    ("primeOnAttach", "primeOnAttach"),
    ("primeOnPlace", "primeOnPlace"),
    ("primeOnPlaceAttach", "primeOnPlaceAttach"),
    ("primeOnTake", "primeOnTake"),
    ("primeOnThrow", "primeOnThrow"),
    ("unprimeOnTake", "unprimeOnTake"),
];

pub(super) fn compare_device<'source>(
    trace: &Trace<'_, 'source>,
    device: &StructValue<'source>,
    exported: &Value,
    counts: &mut Counts,
) -> TestResult {
    for name in ["triggers", "aiTriggers"] {
        let carrier = cell(exported, name)?;
        let Some(original) = device.field(name) else {
            require_state(carrier, "absent")?;
            continue;
        };
        match original {
            Original::Array(values) => {
                require_state(carrier, "present")?;
                trace.check(carrier, original, "selector")?;
                let exported = carrier["value"]
                    .as_array()
                    .ok_or("exported device trigger array")?;
                assert_eq!(
                    exported.len(),
                    values.len(),
                    "actual physical trigger slots"
                );
                for (original, projected) in values.iter().zip(exported) {
                    counts.scalars += compare_trigger(trace, original, projected)?;
                    counts.triggers += 1;
                }
            }
            other => compare_non_object(trace, other, carrier)?,
        }
    }
    let death = cell(exported, "deathBehavior")?;
    if let Some(original) = device.field("deathBehavior") {
        counts.scalars += compare_behavior(trace, original, death)?;
    } else {
        require_state(death, "absent")?;
    }
    Ok(())
}

fn compare_trigger<'source>(
    trace: &Trace<'_, 'source>,
    original: &Original<'source>,
    exported: &Value,
) -> TestResult<usize> {
    let Original::Struct(trigger) = original else {
        compare_non_object(trace, original, exported)?;
        return Ok(0);
    };
    require_state(exported, "present")?;
    trace.part(exported, original, "$type", "selector")?;
    assert_eq!(
        proof::text(exported, "/value/sourceType/value")?,
        trigger.type_name(),
        "typed trigger preserves the exact concrete declaration"
    );
    let export = &exported["value"];
    let mut count = compare_fields(
        trace,
        trigger,
        export,
        &[("isAuthoritative", "isAuthoritative")],
    )?;
    count += compare_trigger_variant(trace, trigger, export)?;
    let behavior = cell(export, "behavior")?;
    if let Some(original) = trigger.field("behavior") {
        count += compare_behavior(trace, original, behavior)?;
    } else {
        require_state(behavior, "absent")?;
    }
    Ok(count)
}

fn compare_trigger_variant<'source>(
    trace: &Trace<'_, 'source>,
    trigger: &StructValue<'source>,
    exported: &Value,
) -> TestResult<usize> {
    let (kind, fields, payload) = match trigger.type_name() {
        "STriggerableDevicesTriggerTimerParams" => ("timer", "duration", exported),
        "STriggerableDevicesTriggerImpactParams" => ("impact", "ignoreWaterCollision", exported),
        "STriggerableDevicesTriggerLaserTripParams" => {
            ("laserTrip", "laserLength", &exported["value"])
        }
        _ => {
            assert_eq!(exported["type"], "other", "future readable subtype");
            return Ok(0);
        }
    };
    assert_eq!(
        exported["type"], kind,
        "typed discriminator reflects the original concrete declaration"
    );
    compare_fields(trace, trigger, payload, &[(fields, fields)])
}

fn compare_behavior<'source>(
    trace: &Trace<'_, 'source>,
    original: &Original<'source>,
    exported: &Value,
) -> TestResult<usize> {
    let Original::Struct(behavior) = original else {
        compare_non_object(trace, original, exported)?;
        return Ok(0);
    };
    require_state(exported, "present")?;
    trace.part(exported, original, "$type", "selector")?;
    assert_eq!(
        proof::text(exported, "/value/sourceType/value")?,
        behavior.type_name(),
        "typed behavior preserves the exact concrete declaration"
    );
    let export = &exported["value"];
    let mut count = compare_fields(
        trace,
        behavior,
        export,
        &[("shouldBeDestroyed", "shouldBeDestroyed")],
    )?;
    if behavior.type_name() == "STriggerableDevicesBehaviorExplosionParams" {
        assert_eq!(
            export["type"], "explosion",
            "discriminator matches the original explosion shape"
        );
        let blast = cell(export, "explosionParams")?;
        let Some(original) = behavior.field("explosionParams") else {
            require_state(blast, "absent")?;
            return Ok(count);
        };
        if let Original::Struct(parameters) = original {
            require_state(blast, "present")?;
            trace.part(blast, original, "$type", "selector")?;
            count += compare_fields(
                trace,
                parameters,
                &blast["value"],
                &[("pressure", "pressure")],
            )?;
        } else {
            compare_non_object(trace, original, blast)?;
        }
    }
    Ok(count)
}

fn compare_non_object<'source>(
    trace: &Trace<'_, 'source>,
    original: &Original<'source>,
    exported: &Value,
) -> TestResult {
    match original {
        Original::Null => {
            require_state(exported, "null")?;
            trace.check(exported, original, "selector")
        }
        Original::Error(_) => {
            require_state(exported, "unreadable")?;
            trace.attempted_value(exported, original)
        }
        _ => Err("unexpected actual trigger device storage".into()),
    }
}
