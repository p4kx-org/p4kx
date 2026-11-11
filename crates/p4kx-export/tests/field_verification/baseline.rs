// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::compare::Rule;
use super::inventory::{Disposition, FieldKey, Inventory, Mapping};
use super::path::{Path, Step};
use super::published_model::{Family, Field};
use super::units::Unit;

fn mapped(owner: &str, field: &str, block: &str, target: &str) -> Mapping {
    Mapping {
        source: FieldKey::new(owner, field),
        declared_target: None,
        disposition: Disposition::Exported {
            group: "items".to_owned(),
            path: Path::fields(&["stats", block, target]),
        },
        previously_empty: false,
    }
}

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

pub(crate) fn inventory() -> Inventory {
    let mut fields: Vec<_> = shield_mappings().collect();
    for (source, target) in [
        ("quantumFuelRequirement", "fuelRequirement"),
        ("jumpRange", "jumpRange"),
        ("disconnectRange", "disconnectRange"),
    ] {
        fields.push(mapped(
            "SCItemQuantumDriveParams",
            source,
            "quantumDrive",
            target,
        ));
    }
    let mut params = mapped(
        "SCItemQuantumDriveParams",
        "params",
        "quantumDrive",
        "driveSpeed",
    );
    params.declared_target = Some("QuantumDriveParams".to_owned());
    fields.push(params);
    for name in [
        "driveSpeed",
        "cooldownTime",
        "spoolUpTime",
        "stageOneAccelRate",
        "stageTwoAccelRate",
        "engageSpeed",
    ] {
        fields.push(mapped("QuantumDriveParams", name, "quantumDrive", name));
    }
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
