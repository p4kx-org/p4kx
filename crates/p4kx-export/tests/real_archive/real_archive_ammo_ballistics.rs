// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;
use serde_json::Value;

use super::{CheckedStruct, Result, check_discriminator, fields, structure};
use crate::real_archive_item_components::cell;
use crate::real_archive_item_durability::Trace;

const PENETRATION: &[(&str, &str)] = &[("AmmoPenetrationParams", "penetration")];
const FALL_OFF: &[(&str, &str)] = &[("BulletDamageDropParams", "falloff")];
const IMPULSE: &[(&str, &str)] = &[("BulletImpulseFalloffParams", "impulse")];
const PIERCE: &[(&str, &str)] = &[("BulletPierceabilityParams", "pierce")];
const ELECTRON: &[(&str, &str)] = &[("BulletElectronParams", "electron")];
const DAMAGE: &[(&str, &str)] = &[
    ("DamageBase", "base"),
    ("DamageInfo", "info"),
    ("DamageParams", "parameters"),
];
const CHANNELS: &[(&str, &str)] = &[
    ("DamagePhysical", "physical"),
    ("DamageEnergy", "energy"),
    ("DamageDistortion", "distortion"),
    ("DamageThermal", "thermal"),
    ("DamageBiochemical", "biochemical"),
    ("DamageStun", "stun"),
];

pub(super) fn ballistic<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
) -> Result<usize> {
    Ok(damage(trace, original, row, "damage")?
        + compare_group(
            trace,
            original,
            row,
            "penetrationParams",
            PENETRATION,
            &[
                ("basePenetrationDistance", "basePenetrationDistance"),
                ("nearRadius", "nearRadius"),
                ("farRadius", "farRadius"),
            ],
        )?)
}

pub(super) fn bullet<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
) -> Result<usize> {
    let mut count = ballistic(trace, original, row)?;
    count += damage_drop(trace, original, row)?;
    count += compare_group(
        trace,
        original,
        row,
        "impulseFalloffParams",
        IMPULSE,
        &[
            ("minDistance", "minDistance"),
            ("dropFalloff", "dropFalloff"),
            ("maxFalloff", "maxFalloff"),
        ],
    )?;
    count += compare_group(
        trace,
        original,
        row,
        "pierceabilityParams",
        PIERCE,
        &[
            ("damageFalloffLevel1", "damageFalloffLevel1"),
            ("damageFalloffLevel2", "damageFalloffLevel2"),
            ("damageFalloffLevel3", "damageFalloffLevel3"),
            ("maxPenetrationThickness", "maxPenetrationThickness"),
        ],
    )?;
    count += compare_group(
        trace,
        original,
        row,
        "electronParams",
        ELECTRON,
        &[
            ("jumpRange", "jumpRange"),
            ("maximumJumps", "maximumJumps"),
            ("residualChargeMultiplier", "residualChargeMultiplier"),
        ],
    )?;
    Ok(count)
}

fn damage_drop<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
) -> Result<usize> {
    let Some(source) = group(trace, original, row, "damageDropParams", FALL_OFF)? else {
        return Ok(0);
    };
    let mut count = 0;
    for name in [
        "damageDropMinDistance",
        "damageDropPerMeter",
        "damageDropMinDamage",
    ] {
        count += damage(trace, source.original, source.json, name)?;
    }
    Ok(count)
}

pub(super) fn damage<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
    name: &str,
) -> Result<usize> {
    let Some(source) = group(trace, original, row, name, DAMAGE)? else {
        return Ok(0);
    };
    check_discriminator(source.json, source.kind)?;
    match source.kind {
        "base" => Ok(0),
        "info" => fields(
            trace,
            source.original,
            cell(source.json, "value")?,
            CHANNELS,
        ),
        "parameters" => fields(
            trace,
            source.original,
            cell(source.json, "value")?,
            &[("damageTotal", "damageTotal")],
        ),
        _ => Err("unverified damage type".into()),
    }
}

fn compare_group<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
    name: &str,
    types: &[(&str, &'static str)],
    mappings: &[(&str, &str)],
) -> Result<usize> {
    let Some(source) = group(trace, original, row, name, types)? else {
        return Ok(0);
    };
    fields(trace, source.original, source.json, mappings)
}

pub(super) fn group<'tree, 'source, 'row>(
    trace: &Trace<'_, 'source>,
    original: &'tree StructValue<'source>,
    row: &'row Value,
    name: &str,
    types: &[(&str, &'static str)],
) -> Result<Option<CheckedStruct<'tree, 'source, 'row>>> {
    structure(trace, original, row, name, name, types)
}
