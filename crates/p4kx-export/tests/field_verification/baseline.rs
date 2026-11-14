// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::compare::Rule;
use super::inventory::{Disposition, FieldKey, Inventory, Mapping};
use super::path::{Path, Step};
use super::published_model::{Family, Field};
use super::units::Unit;

fn shield_mappings() -> impl Iterator<Item = Mapping> {
    let outer = [
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
    .map(|(source, target)| {
        let declared_target = match source {
            "ShieldResistance" => Some("SShieldResistance"),
            "ShieldAbsorption" => Some("SShieldAbsorption"),
            "stunParams" => Some("SShieldStunParams"),
            _ => None,
        };
        mapped(
            "SCItemShieldGeneratorParams",
            source,
            super::coverage::SHIELD_GENERATORS,
            Path::fields(&[target, "value"]),
            declared_target,
        )
    });
    outer.chain(shield_nested_mappings())
}

fn shield_nested_mappings() -> impl Iterator<Item = Mapping> {
    let stun = [
        "minAlphaDamageRatio",
        "maxAlphaDamageRatio",
        "minStunTime",
        "maxStunTime",
    ]
    .into_iter()
    .map(|name| {
        mapped(
            "SShieldStunParams",
            name,
            super::coverage::SHIELD_GENERATORS,
            Path::fields(&["stunParams", "value", name, "value"]),
            None,
        )
    });
    let ranges = [
        ("SShieldAbsorption", "shieldAbsorption"),
        ("SShieldResistance", "shieldResistance"),
    ]
    .into_iter()
    .flat_map(|(owner, group)| {
        [("Min", "min"), ("Max", "max")]
            .into_iter()
            .map(move |(source, target)| {
                let mut path = Path::fields(&[group, "value"]);
                path.0.push(Step::Each);
                path.0.extend(Path::fields(&["value", target, "value"]).0);
                mapped(
                    owner,
                    source,
                    super::coverage::SHIELD_GENERATORS,
                    path,
                    None,
                )
            })
    });
    stun.chain(ranges)
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
        mapped(
            "SCItemQuantumDriveParams",
            source,
            super::coverage::QUANTUM_DRIVES,
            Path::fields(&[target, "value"]),
            match source {
                "params" | "splineJumpParams" => Some(super::coverage::QUANTUM_PROFILE_TYPE),
                "heatParams" => Some("QuantumDriveHeatParams"),
                _ => None,
            },
        )
    });
    roots
        .chain(quantum_profile_mappings())
        .chain(quantum_heat_mappings())
        .chain(std::iter::once(quantum_effect_reference()))
        .chain(quantum_arena_roots())
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
        mapped(
            super::coverage::QUANTUM_PROFILE_TYPE,
            source,
            super::coverage::QUANTUM_PROFILES,
            Path::fields(&[target, "value"]),
            match source {
                "idleState" | "travellingState" => Some("ItemResourceState"),
                "effectTags" => Some("SQuantumDriveEffectBaseParams"),
                _ => None,
            },
        )
    })
}

fn quantum_heat_mappings() -> impl Iterator<Item = Mapping> {
    [
        "preRampUpThermalEnergyDraw",
        "rampUpThermalEnergyDraw",
        "inFlightThermalEnergyDraw",
        "rampDownThermalEnergyDraw",
        "postRampDownThermalEnergyDraw",
    ]
    .into_iter()
    .map(|name| {
        mapped(
            "QuantumDriveHeatParams",
            name,
            super::coverage::QUANTUM_DRIVES,
            Path::fields(&["heat", "value", name, "value"]),
            None,
        )
    })
}

fn quantum_effect_reference() -> Mapping {
    const OWNER: &str = "SQuantumDriveEffectTemplateRef";
    const FIELD: &str = "tagRecord";
    let mut path = Path::fields(&["effectTags", "value"]);
    path.0.push(Step::OwnedArenaReference {
        owner: OWNER.to_owned(),
        name: FIELD.to_owned(),
    });
    path.0.extend(Path::fields(&["value", "guid", "value"]).0);
    mapped(
        OWNER,
        FIELD,
        super::coverage::QUANTUM_PROFILES,
        path,
        Some("SQuantumDriveEffectTemplate"),
    )
}

fn quantum_arena_roots() -> impl Iterator<Item = Mapping> {
    [
        (
            "SCItemQuantumDriveParams",
            "tracePoint",
            super::coverage::QUANTUM_DRIVES,
            None,
        ),
        (
            super::coverage::QUANTUM_PROFILE_TYPE,
            "beginQuantumTravelInteraction",
            super::coverage::QUANTUM_PROFILES,
            Some("SSharedInteractionParams"),
        ),
        (
            super::coverage::QUANTUM_PROFILE_TYPE,
            "abortQuantumTravelInteraction",
            super::coverage::QUANTUM_PROFILES,
            Some("SSharedInteractionParams"),
        ),
    ]
    .into_iter()
    .map(|(owner, field, group, declared_target)| {
        let mut path = Path::fields(&["additionalFields"]);
        path.0.push(Step::ArenaRootMember {
            name: field.to_owned(),
        });
        mapped(owner, field, group, path, declared_target)
    })
}

fn mapped(
    owner: &str,
    field: &str,
    group: &str,
    path: Path,
    declared_target: Option<&str>,
) -> Mapping {
    Mapping {
        source: FieldKey::new(owner, field),
        declared_target: declared_target.map(str::to_owned),
        disposition: Disposition::Exported {
            group: group.to_owned(),
            path,
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
