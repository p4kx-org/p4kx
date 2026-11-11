// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::compare::Rule;
use super::inventory::{Disposition, FieldKey, Inventory, Mapping};
use super::path::{Path, Step};
use super::published_model::{Family, Field};
use super::units::Unit;

fn shield_mappings() -> impl Iterator<Item = Mapping> {
    [
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
    ]
    .into_iter()
    .map(|(source, target)| Mapping {
        source: FieldKey::new("SCItemShieldGeneratorParams", source),
        declared_target: None,
        disposition: Disposition::Exported {
            group: super::coverage::SHIELD_GENERATORS.to_owned(),
            path: Path::fields(&[target, "value"]),
        },
        previously_empty: false,
    })
}

fn quantum_mappings() -> impl Iterator<Item = Mapping> {
    let roots = [
        ("quantumFuelRequirement", "fuelRequirement"),
        ("jumpRange", "jumpRange"),
        ("disconnectRange", "disconnectRange"),
        ("params", "standard"),
        ("splineJumpParams", "spline"),
        ("heatParams", "heat"),
        ("quantumBoostParams", "boost"),
    ]
    .into_iter()
    .map(|(source, target)| {
        quantum_mapped(
            "SCItemQuantumDriveParams",
            source,
            super::coverage::QUANTUM_DRIVES,
            target,
            (source == "params").then_some(super::coverage::QUANTUM_PROFILE_TYPE),
        )
    });
    roots.chain(quantum_profile_mappings())
}

fn quantum_profile_mappings() -> impl Iterator<Item = Mapping> {
    [
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
        ("idleState", "idleState"),
        ("travellingState", "travellingState"),
        ("effectTags", "effectTags"),
    ]
    .into_iter()
    .map(|(source, target)| {
        quantum_mapped(
            super::coverage::QUANTUM_PROFILE_TYPE,
            source,
            super::coverage::QUANTUM_PROFILES,
            target,
            None,
        )
    })
}

fn quantum_mapped(
    owner: &str,
    field: &str,
    group: &str,
    target: &str,
    declared_target: Option<&str>,
) -> Mapping {
    Mapping {
        source: FieldKey::new(owner, field),
        declared_target: declared_target.map(str::to_owned),
        disposition: Disposition::Exported {
            group: group.to_owned(),
            path: Path::fields(&[target, "value"]),
        },
        previously_empty: false,
    }
}

pub(crate) fn inventory() -> Inventory {
    let fields = shield_mappings().chain(quantum_mappings()).collect();
    Inventory {
        schema_version: 1,
        roots: vec![
            "SCItemShieldGeneratorParams".to_owned(),
            "SCItemQuantumDriveParams".to_owned(),
        ],
        fields,
    }
}

pub(crate) fn raw_mappings() -> Vec<Field> {
    vec![
        Field {
            source: FieldKey::new("SCItemShieldGeneratorParams", "MaxShieldHealth"),
            family: Family::Items,
            reference: Path::fields(&[
                "Raw",
                "Entity",
                "Components",
                "SCItemShieldGeneratorParams",
                "MaxShieldHealth",
            ]),
            catalog: Path::fields(&["stats", "shield", "maxHealth"]),
            reference_unit: Unit::Dimensionless,
            catalog_unit: Unit::Dimensionless,
            rule: Rule::F32RoundTrip,
            resource_join: None,
        },
        Field {
            source: FieldKey::new("SWeaponActionFireRapidParams", "fireRate"),
            family: Family::Items,
            reference: Path::fields(&[
                "Raw",
                "Entity",
                "Components",
                "SCItemWeaponComponentParams",
                "fireActions",
                "SWeaponActionFireRapidParams",
                "fireRate",
            ]),
            catalog: Path(vec![
                Step::Field {
                    name: "stats".to_owned(),
                },
                Step::Field {
                    name: "weapon".to_owned(),
                },
                Step::Field {
                    name: "fireActions".to_owned(),
                },
                Step::Matching {
                    field: "kind".to_owned(),
                    value: "SWeaponActionFireRapidParams".to_owned(),
                },
                Step::Field {
                    name: "fireRate".to_owned(),
                },
            ]),
            reference_unit: Unit::RoundsPerMinute,
            catalog_unit: Unit::RoundsPerMinute,
            rule: Rule::F32RoundTrip,
            resource_join: None,
        },
    ]
}
