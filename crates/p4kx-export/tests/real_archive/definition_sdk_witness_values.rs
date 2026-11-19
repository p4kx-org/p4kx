// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::component_definitions::{
    ConsumableEffectParameters as EffectParameters, ConsumableSubtype, ConsumableType,
    DamageResistanceMacro, MiningLaserGlobalDefinition, TurretHealthModifier,
};
use p4kx_export::catalog_queries::{DefinitionStatus, ResourceFloat32, SourceRecordReference};
use p4kx_extraction::{RecordTree, SourceValueState, SourcedValue, Value as Original, ValueError};

use super::{TestResult, Trace};

pub(super) fn mining<'source>(
    row: &MiningLaserGlobalDefinition<'_>,
    original: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let mut positive = false;
    for (field, cell) in [
        ("throttleChangePerAction", row.throttle_change_per_action()),
        ("throttleAccPeriod", row.throttle_acc_period()),
        ("throttleAccFactor", row.throttle_acc_factor()),
        ("throttleHoldAccFactor", row.throttle_hold_acc_factor()),
    ] {
        positive |= single(cell, original.field(field), trace)?;
    }
    for (field, cell) in [
        (
            "blockThrottleChangeWhenNotFiring",
            row.block_throttle_change_when_not_firing(),
        ),
        ("throttleResetOnStopFire", row.throttle_reset_on_stop_fire()),
    ] {
        match original.field(field) {
            Some(raw @ Original::Bool(value)) => {
                assert!(
                    cell.value() == Some(value),
                    "exact authored throttle toggle"
                );
                trace.check(&serde_json::to_value(cell)?, raw, "payload")?;
            }
            raw => absent_or_null(cell, raw)?,
        }
    }
    mining_audio(row, original, trace)?;
    assert!(
        positive,
        "MiningLaserGlobalParams must exercise authored throttle payload"
    );
    Ok(())
}

fn mining_audio<'source>(
    row: &MiningLaserGlobalDefinition<'_>,
    original: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    for (field, cell) in [
        ("throttleRTPC", row.throttle_rtpc()),
        (
            "mineableGlowStrengthRTPC",
            row.mineable_glow_strength_rtpc(),
        ),
        (
            "mineableOptimalBreakZoneRTPC",
            row.mineable_optimal_break_zone_rtpc(),
        ),
    ] {
        match original.field(field) {
            Some(Original::Struct(raw)) => {
                let control = cell.value().ok_or("a retained authored RTPC control")?;
                match raw.field("rtpc") {
                    Some(raw @ Original::String(value)) => {
                        let name = control.rtpc();
                        assert!(
                            name.value().map(String::as_str) == Some(*value),
                            "exact original RTPC string"
                        );
                        trace.check(&serde_json::to_value(name)?, raw, "payload")?;
                    }
                    raw => absent_or_null(control.rtpc(), raw)?,
                }
            }
            raw => absent_or_null(cell, raw)?,
        }
    }
    Ok(())
}

pub(super) fn consumable_type<'source>(
    row: &ConsumableType<'_>,
    original: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    text(row.type_name(), original.field("typeName"), trace)?;
    let source = original.field("subtypes");
    let (raw, slots) = selected_array(row.subtypes(), source, trace)?;
    let mut good = false;
    for (raw, cell) in raw.iter().zip(slots) {
        good |= subtype_link(cell, raw, trace)?;
    }
    assert!(
        good,
        "ConsumableType must exercise at least one resolved subtype Reference"
    );
    Ok(())
}

fn subtype_link<'source>(
    cell: &SourcedValue<Box<SourceRecordReference<'_>>>,
    raw: &Original<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<bool> {
    match raw {
        Original::Reference(reference) => {
            let actual = cell
                .value()
                .ok_or("the original subtype Reference is retained")?;
            assert!(
                actual.guid().value() == Some(&reference.guid()),
                "original subtype GUID in its ordered slot"
            );
            assert!(
                actual.resolved_index() == Some(reference.record_index()),
                "physical original subtype target"
            );
            trace.check(&serde_json::to_value(cell)?, raw, "selector")?;
            Ok(true)
        }
        Original::Error(ValueError::DanglingReference { guid }) => {
            let actual = cell
                .value()
                .ok_or("the dangling original subtype Reference is retained")?;
            assert!(
                actual.guid().value() == Some(guid),
                "dangling subtype GUID retains its slot"
            );
            assert!(
                matches!(actual.status(), DefinitionStatus::Dangling),
                "explicit dangling Reference"
            );
            trace.part(&serde_json::to_value(cell)?, raw, "$error.guid", "selector")?;
            Ok(false)
        }
        _ => {
            absent_or_null(cell, Some(raw))?;
            trace.check(&serde_json::to_value(cell)?, raw, "selector")?;
            Ok(false)
        }
    }
}

pub(super) fn armor<'source>(
    row: &DamageResistanceMacro<'_>,
    original: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Some(Original::Struct(raw)) = original.field("damageResistance") else {
        return Err("select a DamageResistanceMacro with an original authored body".into());
    };
    let body = row
        .damage_resistance()
        .value()
        .ok_or("typed resistance body")?;
    let mut positive = false;
    for (field, cell) in [
        ("PhysicalResistance", body.channels().physical()),
        ("EnergyResistance", body.channels().energy()),
        ("DistortionResistance", body.channels().distortion()),
        ("ThermalResistance", body.channels().thermal()),
        ("BiochemicalResistance", body.channels().biochemical()),
        ("StunResistance", body.channels().stun()),
    ] {
        let original = raw.field(field);
        match original {
            Some(Original::Struct(authored)) => {
                let typed = cell.value().ok_or("retained typed resistance channel")?;
                positive |= single(typed.multiplier(), authored.field("Multiplier"), trace)?;
                single(typed.threshold(), authored.field("Threshold"), trace)?;
                single(typed.damage_cap(), authored.field("DamageCap"), trace)?;
                single(
                    typed.damage_cap_limit(),
                    authored.field("DamageCapLimit"),
                    trace,
                )?;
            }
            raw => absent_or_null(cell, raw)?,
        }
    }
    assert!(
        positive,
        "the resistance macro exercises original multiplier bits"
    );
    Ok(())
}

pub(super) fn turret<'source>(
    row: &TurretHealthModifier<'_>,
    original: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let Some(Original::Struct(raw)) = original.field("damageMovementModifier") else {
        return Err("select a turret modifier with an original damage movement curve".into());
    };
    let curve = row
        .damage_movement_modifier()
        .value()
        .ok_or("typed original curve")?;
    let (raw, slots) = selected_array(curve.points(), raw.field("points"), trace)?;
    let mut positive = false;
    for (raw, cell) in raw.iter().zip(slots) {
        match raw {
            Original::Struct(authored) => {
                let point = cell.value().ok_or("typed original curve knot")?;
                positive |= single(point.x(), authored.field("x"), trace)?;
                single(point.y(), authored.field("y"), trace)?;
            }
            raw => absent_or_null(cell, Some(raw))?,
        }
    }
    assert!(
        positive,
        "the modifier curve exercises an authored knot payload"
    );
    Ok(())
}

pub(super) fn consumable_subtype<'source>(
    row: &ConsumableSubtype<'_>,
    original: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    text(row.type_name(), original.field("typeName"), trace)?;
    let (raw, slots) = selected_array(row.effects(), original.field("effectsPerMicroSCU"), trace)?;
    let mut positive = false;
    for (raw, cell) in raw.iter().zip(slots) {
        match raw {
            Original::Struct(authored) => {
                let effect = cell.value().ok_or("typed original consumable effect")?;
                assert!(
                    effect.source().source_type().value() == Some(&authored.type_name()),
                    "concrete authored effect at the exact list position"
                );
                positive |= match (authored.type_name(), effect.parameters()) {
                    ("ConsumableEffectHealth", EffectParameters::Health { health_change }) => {
                        single(health_change, authored.field("healthChange"), trace)?
                    }
                    ("ConsumableEffectResource", EffectParameters::Resource { amount, .. }) => {
                        single(amount, authored.field("amount"), trace)?
                    }
                    (
                        "ConsumableEffectModifyActorStatus",
                        EffectParameters::ModifyActorStatus {
                            stat_point_change,
                            stat_cooldown_change,
                            ..
                        },
                    ) => {
                        let point =
                            single(stat_point_change, authored.field("statPointChange"), trace)?;
                        single(
                            stat_cooldown_change,
                            authored.field("statCooldownChange"),
                            trace,
                        )? || point
                    }
                    (
                        "ConsumableEffectHealth"
                        | "ConsumableEffectResource"
                        | "ConsumableEffectModifyActorStatus",
                        _,
                    ) => return Err("wrong typed parameter family for the original effect".into()),
                    _ => false,
                };
            }
            raw => absent_or_null(cell, Some(raw))?,
        }
    }
    assert!(
        positive,
        "ConsumableSubtype must exercise an authored scalar effect"
    );
    Ok(())
}

fn selected_array<'row, 'source, Row: serde::Serialize>(
    cell: &'row SourcedValue<Vec<SourcedValue<Row>>>,
    original: Option<&'row Original<'source>>,
    trace: &Trace<'_, 'source>,
) -> TestResult<(&'row [Original<'source>], &'row [SourcedValue<Row>])> {
    let Some(value @ Original::Array(raw)) = original else {
        return Err("select a witness with an original array".into());
    };
    let actual = cell.value().ok_or("retained typed array")?;
    assert!(!raw.is_empty(), "the source witness has a populated array");
    assert!(
        raw.len() == actual.len(),
        "one typed slot per original array occurrence"
    );
    trace.check(&serde_json::to_value(cell)?, value, "selector")?;
    Ok((raw, actual))
}

fn single<'source>(
    cell: &SourcedValue<ResourceFloat32>,
    raw: Option<&Original<'source>>,
    trace: &Trace<'_, 'source>,
) -> TestResult<bool> {
    match raw {
        Some(raw @ Original::Single(number)) => {
            assert!(
                cell.value().map(ResourceFloat32::raw_bits) == Some(number.to_bits()),
                "exact authored Single bits"
            );
            trace.check(&serde_json::to_value(cell)?, raw, "payload")?;
            Ok(true)
        }
        raw => {
            absent_or_null(cell, raw)?;
            Ok(false)
        }
    }
}

fn text<'source>(
    cell: &SourcedValue<&str>,
    raw: Option<&Original<'source>>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    match raw {
        Some(raw @ Original::String(label)) => {
            assert!(cell.value() == Some(label), "exact authored string");
            trace.check(&serde_json::to_value(cell)?, raw, "payload")
        }
        raw => absent_or_null(cell, raw),
    }
}

fn absent_or_null<T>(cell: &SourcedValue<T>, original: Option<&Original<'_>>) -> TestResult {
    let state = match original {
        None => SourceValueState::Absent,
        Some(Original::Null) => SourceValueState::Null,
        _ => return Err("unexpected original value kind in the selected witness".into()),
    };
    assert!(
        cell.state() == state && cell.value().is_none(),
        "exact absent or null source state"
    );
    Ok(())
}
