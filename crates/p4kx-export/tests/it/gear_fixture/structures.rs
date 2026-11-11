// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn singles(name: &str, fields: &[&str]) -> TestStruct {
    fields
        .iter()
        .fold(TestStruct::new(name), |structure, field| {
            structure.field(*field, TestType::Single)
        })
}

fn worn() -> Vec<TestStruct> {
    vec![
        TestStruct::new("EntityClassDefinition")
            .array("Components", pointer("DataForgeComponentParams")),
        TestStruct::new("DataForgeComponentParams"),
        component("SAttachableComponentParams").field("AttachDef", class("SItemDefinition")),
        TestStruct::new("SItemDefinition").field("Type", TestType::Enum("EItemType".to_owned())),
        component("SCItemSuitArmorParams")
            .field("integrityMilestoneToBreak", TestType::Single)
            .field("damageResistance", TestType::Reference),
        TestStruct::new("DamageResistanceMacro")
            .field("damageResistance", class("DamageResistance")),
        TestStruct::new("DamageResistance")
            .field("IgnoreMeleeDamage", TestType::Bool)
            .field("PhysicalResistance", class("DamageResistanceEntry"))
            .field("EnergyResistance", class("DamageResistanceEntry")),
        singles(
            "DamageResistanceEntry",
            &["Multiplier", "Threshold", "DamageCap", "DamageCapLimit"],
        ),
        component("SCItemClothingParams")
            .field("TemperatureResistance", class("ClothingTemperature"))
            .field("RadiationResistance", class("ClothingRadiation"))
            .field("Flight", class("ClothingFlight")),
        singles("ClothingTemperature", &["MinResistance", "MaxResistance"]),
        singles(
            "ClothingRadiation",
            &["MaximumRadiationCapacity", "RadiationDissipationRate"],
        ),
        singles("ClothingFlight", &["gForceResistance"]),
        component("SCItemRadarComponentParams")
            .field("aimAssist", class("RadarAimAssist"))
            .field("pingProperties", class("RadarPing")),
        singles(
            "RadarAimAssist",
            &[
                "distanceMinAssignment",
                "distanceMaxAssignment",
                "outsideRangeBufferDistance",
            ],
        ),
        singles("RadarPing", &["cooldownTime"]),
    ]
}

fn weapons() -> Vec<TestStruct> {
    vec![
        component("SCItemWeaponComponentParams")
            .array("fireActions", pointer("SWeaponActionParams")),
        TestStruct::new("SWeaponActionParams"),
        singles(
            "SWeaponActionFireTractorBeamParams",
            &[
                "minForce",
                "maxForce",
                "minDistance",
                "maxDistance",
                "fullStrengthDistance",
                "maxVolume",
                "maxAngle",
            ],
        )
        .parent("SWeaponActionParams"),
        TestStruct::new("SWeaponActionToggleParams")
            .parent("SWeaponActionParams")
            .field("weaponAction", pointer("SWeaponActionParams")),
        TestStruct::new("SWeaponActionParallelParams")
            .parent("SWeaponActionParams")
            .array("weaponActions", pointer("SWeaponActionParams")),
        TestStruct::new("SWeaponActionFireBeamParams")
            .parent("SWeaponActionParams")
            .field("hitType", TestType::String)
            .field("fullDamageRange", TestType::Single)
            .field("zeroDamageRange", TestType::Single)
            .field("damagePerSecond", class("DamageInfo")),
        singles("DamageInfo", &["DamagePhysical", "DamageEnergy"]),
        component("EntityComponentAttachableModifierParams")
            .field("activationMethod", TestType::String)
            .field("charges", TestType::Int32)
            .field("canInterrupt", TestType::Bool)
            .field("isInterruptible", TestType::Bool)
            .array("modifiers", pointer("ItemModifiersParams")),
        TestStruct::new("ItemModifiersParams"),
        TestStruct::new("ItemWeaponModifiersParams")
            .parent("ItemModifiersParams")
            .field("fireActionIndex", TestType::Int32)
            .field("modifierLifetime", pointer("ItemModifierTimedLife"))
            .field("weaponModifier", pointer("SWeaponModifierParams")),
        TestStruct::new("SWeaponModifierParams").field("weaponStats", class("SWeaponStats")),
        TestStruct::new("SWeaponStats")
            .field("fireRateMultiplier", TestType::Single)
            .field("damageMultiplier", TestType::Single)
            .field("salvageModifier", pointer("SSalvageModifier"))
            .field("tractorModifier", pointer("STractorModifier")),
    ]
}

fn multipliers() -> Vec<TestStruct> {
    vec![
        singles(
            "SSalvageModifier",
            &[
                "salvageSpeedMultiplier",
                "radiusMultiplier",
                "extractionEfficiency",
            ],
        ),
        singles(
            "STractorModifier",
            &[
                "forceMultiplier",
                "fullStrengthDistMultiplier",
                "maxDistMultiplier",
                "maxVolumeMultiplier",
            ],
        ),
    ]
}

fn mining() -> Vec<TestStruct> {
    let modifier = |name: &str, field: &str| {
        TestStruct::new(name)
            .parent("ItemModifiersParams")
            .field("modifierLifetime", pointer("ItemModifierTimedLife"))
            .field(field, class("MiningLaserModifiers"))
    };
    vec![
        component("SEntityComponentMiningLaserParams")
            .field("throttleLerpSpeed", TestType::Single)
            .field("throttleMinimum", TestType::Single)
            .field("usesPowerThrottle", TestType::Bool)
            .field("globalParams", TestType::Reference)
            .field("miningLaserModifiers", class("MiningLaserModifiers"))
            .field("filterParams", class("MiningFilterParams")),
        TestStruct::new("MiningLaserModifiers")
            .field("isOptimalRateGood", TestType::Bool)
            .field("laserInstability", pointer("FloatModifierMultiplicative"))
            .field(
                "optimalChargeWindowSizeModifier",
                pointer("FloatModifierMultiplicative"),
            )
            .field("resistanceModifier", pointer("FloatModifierMultiplicative"))
            .field(
                "shatterdamageModifier",
                pointer("FloatModifierMultiplicative"),
            )
            .field(
                "clusterFactorModifier",
                pointer("FloatModifierMultiplicative"),
            )
            .field(
                "optimalChargeWindowRateModifier",
                pointer("FloatModifierMultiplicative"),
            )
            .field(
                "catastrophicChargeWindowRateModifier",
                pointer("FloatModifierMultiplicative"),
            ),
        singles("FloatModifierMultiplicative", &["value"]),
        TestStruct::new("MiningFilterParams")
            .field("filterModifier", pointer("FloatModifierMultiplicative")),
        singles("ItemModifierTimedLife", &["lifetime"]),
        modifier("ItemMiningModifierParams", "MiningLaserModifier"),
        modifier("ItemMineableRockModifierParams", "MiningLaserModifier"),
        TestStruct::new("MiningFilterItemModifierParams")
            .parent("ItemModifiersParams")
            .field("modifierLifetime", pointer("ItemModifierTimedLife"))
            .field("filterParams", class("MiningFilterParams")),
    ]
}

pub(super) fn database() -> DataForgeBuilder {
    worn()
        .into_iter()
        .chain(weapons())
        .chain(multipliers())
        .chain(mining())
        .fold(
            DataForgeBuilder::new().version(8).enumeration(
                "EItemType",
                [
                    "Char_Armor_Torso",
                    "Char_Clothing_Hat",
                    "Radar",
                    "TractorBeam",
                    "SalvageModifier",
                    "WeaponGun",
                    "WeaponMining",
                    "MiningModifier",
                ],
            ),
            DataForgeBuilder::structure,
        )
}
