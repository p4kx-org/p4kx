// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue, Value as Original};
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_item_components::{cell, require_state};
use crate::real_archive_item_durability::{self as proof, Trace};

#[path = "real_archive_ammo_controller.rs"]
mod controller;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const ROOT: &[(&str, &str)] = &[
    ("spawnType", "spawnType"),
    ("size", "size"),
    ("ammoCategory", "ammoCategory"),
    ("bulletType", "bulletType"),
    ("hitPoints", "hitPoints"),
    ("lifetime", "lifetime"),
    ("speed", "speed"),
    ("impulseScale", "impulseScale"),
    ("noBulletHits", "noBulletHits"),
    ("quietRemoval", "quietRemoval"),
    ("useInConvergence", "useInConvergence"),
    ("shotsPerAudioLoop", "shotsPerAudioLoop"),
];
const PROJECTILES: &[(&str, &str)] = &[
    ("ProjectileParams", "base"),
    ("BulletProjectileParams", "bullet"),
    ("TachyonProjectileParams", "tachyon"),
    ("CounterMeasureProjectileParams", "countermeasure"),
];
const COUNTERMEASURES: &[(&str, &str)] = &[
    ("CounterMeasureBaseParams", "base"),
    ("CounterMeasureChaffParams", "chaff"),
    ("CounterMeasureFlareParams", "flare"),
];
const DETONATION: &[(&str, &str)] = &[("ProjectileDetonationParams", "detonation")];

const DETONATION_FIELDS: &[(&str, &str)] = &[
    ("armTime", "armTime"),
    ("safeDistance", "safeDistance"),
    ("explodeOnImpact", "explodeOnImpact"),
    ("explodeOnFinalImpact", "explodeOnFinalImpact"),
];
const BULLET_FIELDS: &[(&str, &str)] = &[
    ("impactRadius", "impactRadius"),
    ("minImpactRadius", "minImpactRadius"),
    ("ignitionChanceOverride", "ignitionChanceOverride"),
    ("keepAliveOnZeroDamage", "keepAliveOnZeroDamage"),
];
const TACHYON_FIELDS: &[(&str, &str)] = &[
    ("fullDamageRange", "fullDamageRange"),
    ("zeroDamageRange", "zeroDamageRange"),
    ("hitType", "hitType"),
];

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum AuthoredShape<'source> {
    Absent,
    Null,
    Unreadable,
    Invalid,
    Struct(&'source str),
}

impl<'source> AuthoredShape<'source> {
    const fn of(value: Option<&Original<'source>>) -> Self {
        match value {
            None => Self::Absent,
            Some(Original::Null) => Self::Null,
            Some(Original::Error(_)) => Self::Unreadable,
            Some(Original::Struct(value)) => Self::Struct(value.type_name()),
            Some(_) => Self::Invalid,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Shape<'source> {
    projectile: AuthoredShape<'source>,
    countermeasure: AuthoredShape<'source>,
    conversion: AuthoredShape<'source>,
    physics_wrapper: AuthoredShape<'source>,
    physics_type: AuthoredShape<'source>,
}

impl<'source> Shape<'source> {
    pub(super) fn read(tree: &RecordTree<'source>) -> Self {
        let projectile = tree.field("projectileParams");
        let physics_wrapper = tree.field("physicsControllerParams");
        Self {
            projectile: AuthoredShape::of(projectile),
            countermeasure: AuthoredShape::of(child(projectile, "typeParams")),
            conversion: AuthoredShape::of(tree.field("conversionRate")),
            physics_wrapper: AuthoredShape::of(physics_wrapper),
            physics_type: AuthoredShape::of(child(physics_wrapper, "PhysType")),
        }
    }
}

fn child<'tree, 'source>(
    value: Option<&'tree Original<'source>>,
    field: &str,
) -> Option<&'tree Original<'source>> {
    let Some(Original::Struct(value)) = value else {
        return None;
    };
    value.field(field)
}

pub(super) fn compare<'source>(
    trace: &Trace<'_, 'source>,
    tree: &RecordTree<'source>,
    row: &Value,
) -> Result<usize> {
    trace.header(cell(row, "guid")?, "guid", "payload")?;
    trace.header(cell(row, "recordIndex")?, "recordIndex", "selector")?;
    trace.header(cell(row, "sourceType")?, "sourceType", "selector")?;
    Ok(fields(trace, tree.root(), row, ROOT)?
        + projectile(trace, tree.root(), row)?
        + controller::conversion(trace, tree.root(), row)?
        + controller::physics(trace, tree.root(), row)?)
}

fn projectile<'source>(
    trace: &Trace<'_, 'source>,
    root: &StructValue<'source>,
    row: &Value,
) -> Result<usize> {
    let Some(source) = structure(
        trace,
        root,
        row,
        "projectileParams",
        "projectileParams",
        PROJECTILES,
    )?
    else {
        return Ok(0);
    };
    let kind = cell(source.json, "kind")?;
    check_discriminator(kind, source.kind)?;
    let mut count = 0;
    if let Some(detonation) = structure(
        trace,
        source.original,
        source.json,
        "detonationParams",
        "detonationParams",
        DETONATION,
    )? {
        count += fields(
            trace,
            detonation.original,
            detonation.json,
            DETONATION_FIELDS,
        )?;
    }
    Ok(count
        + match source.kind {
            "base" => 0,
            "bullet" => fields(trace, source.original, cell(kind, "value")?, BULLET_FIELDS)?,
            "tachyon" => fields(trace, source.original, cell(kind, "value")?, TACHYON_FIELDS)?,
            "countermeasure" => countermeasure(trace, source.original, cell(kind, "value")?)?,
            _ => return Err("unexpected verified projectile shape".into()),
        })
}

fn countermeasure<'source>(
    trace: &Trace<'_, 'source>,
    original: &StructValue<'source>,
    row: &Value,
) -> Result<usize> {
    let Some(source) = structure(
        trace,
        original,
        row,
        "typeParams",
        "typeParams",
        COUNTERMEASURES,
    )?
    else {
        return Ok(0);
    };
    check_discriminator(source.json, source.kind)?;
    if source.kind == "base" {
        return Ok(0);
    }
    let payload = cell(source.json, "value")?;
    let signals = fields(
        trace,
        source.original,
        payload,
        &[
            ("StartInfrared", "startInfrared"),
            ("EndInfrared", "endInfrared"),
            ("StartElectromagnetic", "startElectromagnetic"),
            ("EndElectromagnetic", "endElectromagnetic"),
            ("StartCrossSection", "startCrossSection"),
            ("EndCrossSection", "endCrossSection"),
            ("StartDecibel", "startDecibel"),
            ("EndDecibel", "endDecibel"),
        ],
    )?;
    if source.kind == "chaff" {
        return Ok(signals
            + fields(
                trace,
                source.original,
                payload,
                &[
                    ("volumeSpawnDelay", "volumeSpawnDelay"),
                    ("volumeLifetime", "volumeLifetime"),
                ],
            )?);
    }
    Ok(signals)
}

struct CheckedStruct<'tree, 'source, 'row> {
    original: &'tree StructValue<'source>,
    json: &'row Value,
    kind: &'static str,
}

fn structure<'tree, 'source, 'row>(
    trace: &Trace<'_, 'source>,
    owner: &'tree StructValue<'source>,
    row: &'row Value,
    original_name: &str,
    name: &str,
    types: &[(&str, &'static str)],
) -> Result<Option<CheckedStruct<'tree, 'source, 'row>>> {
    let carrier = cell(row, name)?;
    let Some(raw) = readable(trace, carrier, owner.field(original_name))? else {
        return Ok(None);
    };
    let Original::Struct(original) = raw else {
        return Err("expected an original owned Struct".into());
    };
    let kind = types
        .iter()
        .find_map(|(name, kind)| (original.type_name() == *name).then_some(*kind));
    trace.part(carrier, raw, "$type", "selector")?;
    if let Some(kind) = kind {
        let json = catalog_v3_consumer::present(carrier)?;
        Ok(Some(CheckedStruct {
            original,
            json,
            kind,
        }))
    } else {
        require_state(carrier, "unsupported")?;
        Ok(None)
    }
}

fn check_discriminator(row: &Value, expected: &str) -> Result<()> {
    assert!(
        proof::text(row, "/type")? == expected,
        "the selected wire branch follows the original concrete type"
    );
    Ok(())
}

fn fields<'source>(
    trace: &Trace<'_, 'source>,
    owner: &StructValue<'source>,
    row: &Value,
    mappings: &[(&str, &str)],
) -> Result<usize> {
    let mut count = 0;
    for &(original, field) in mappings {
        let carrier = cell(row, field)?;
        let Some(raw) = readable(trace, carrier, owner.field(original))? else {
            continue;
        };
        let payload = catalog_v3_consumer::present(carrier)?;
        let equal = match raw {
            Original::Single(value) => single(payload, *value)?,
            Original::Int32(value) => payload.as_i64() == Some(i64::from(*value)),
            Original::UInt8(value) => payload.as_u64() == Some(u64::from(*value)),
            Original::UInt32(value) => payload.as_u64() == Some(u64::from(*value)),
            Original::Bool(value) => payload.as_bool() == Some(*value),
            Original::String(value) | Original::Enum(value) => payload.as_str() == Some(value),
            _ => return Err("unexpected original Ammo scalar storage".into()),
        };
        assert!(equal, "{field} preserves its original scalar and width");
        trace.check(carrier, raw, "payload")?;
        count += 1;
    }
    Ok(count)
}

fn readable<'read, 'source>(
    trace: &Trace<'_, 'source>,
    cell: &Value,
    raw: Option<&'read Original<'source>>,
) -> Result<Option<&'read Original<'source>>> {
    catalog_v3_consumer::source_value(cell)?;
    match raw {
        None => require_state(cell, "absent")?,
        Some(value @ Original::Null) => {
            require_state(cell, "null")?;
            trace.check(cell, value, "selector")?;
        }
        Some(value @ Original::Error(_)) => {
            require_state(cell, "unreadable")?;
            trace.attempted_value(cell, value)?;
        }
        Some(value) => return Ok(Some(value)),
    }
    Ok(None)
}

fn single(value: &Value, raw: f32) -> Result<bool> {
    let word = proof::text(value, "/rawBits")?;
    let bits = u32::from_str_radix(word, 16).map_err(|_| "invalid Single bit word")?;
    let finite = cell(value, "finite")?;
    let decimal_matches = if raw.is_finite() {
        finite.is_number()
            && finite
                .to_string()
                .parse::<f32>()
                .is_ok_and(|number| number.to_bits() == raw.to_bits())
    } else {
        finite.is_null()
    };
    Ok(word.len() == 8 && bits == raw.to_bits() && decimal_matches)
}
