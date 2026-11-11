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

fn singles_component(name: &str, fields: &[&str]) -> TestStruct {
    fields.iter().fold(component(name), |structure, field| {
        structure.field(*field, TestType::Single)
    })
}

fn weapons(loop_shape: TestStruct) -> Vec<TestStruct> {
    [
        TestStruct::new("EntityClassDefinition")
            .array("Components", pointer("DataForgeComponentParams")),
        TestStruct::new("DataForgeComponentParams"),
        component("SAttachableComponentParams").field("AttachDef", class("SItemDefinition")),
        TestStruct::new("SItemDefinition").field("Type", TestType::Enum("EItemType".to_owned())),
        component("SCItemWeaponComponentParams")
            .array("fireActions", pointer("SWeaponActionParams"))
            .field("ammoContainerRecord", TestType::Reference),
        TestStruct::new("SWeaponActionParams"),
        TestStruct::new("SWeaponActionFireRapidParams")
            .parent("SWeaponActionParams")
            .field("name", TestType::String)
            .field("fireRate", TestType::Single)
            .field("heatPerShot", TestType::Single)
            .field("launchParams", class("SProjectileLauncher")),
        TestStruct::new("SWeaponActionFireChargedParams")
            .parent("SWeaponActionParams")
            .field("chargeTime", TestType::Single)
            .field("overchargeTime", TestType::Single)
            .field("cooldownTime", TestType::Single)
            .field("weaponAction", pointer("SWeaponActionParams")),
        TestStruct::new("SProjectileLauncher")
            .field("pelletCount", TestType::Int32)
            .field("pellets", TestType::Int32)
            .field("ammoCost", TestType::Int32)
            .field("spreadParams", class("SSpreadParams")),
        singles(
            "SSpreadParams",
            &["min", "max", "firstAttack", "attack", "decay"],
        ),
        component("SAmmoContainerComponentParams")
            .field("ammoParamsRecord", TestType::Reference)
            .field("maxAmmoCount", TestType::Int32),
        TestStruct::new("AmmoParams")
            .field("speed", TestType::Single)
            .field("lifetime", TestType::Single)
            .field("projectileParams", pointer("BulletProjectileParams"))
            .field("trap", pointer("Loop")),
        loop_shape,
        TestStruct::new("BulletProjectileParams")
            .field("damage", class("DamageInfo"))
            .field("detonationParams", pointer("ProjectileDetonationParams")),
        TestStruct::new("ProjectileDetonationParams")
            .field("explosionParams", class("ExplosionParams")),
        TestStruct::new("ExplosionParams")
            .field("minRadius", TestType::Single)
            .field("maxRadius", TestType::Single)
            .field("damage", class("DamageInfo")),
        singles("DamageInfo", &["DamagePhysical", "DamageEnergy"]),
    ]
    .into()
}

fn components() -> Vec<TestStruct> {
    vec![
        singles_component(
            "SCItemShieldGeneratorParams",
            &[
                "MaxShieldHealth",
                "MaxShieldRegen",
                "DecayRatio",
                "DownedRegenDelay",
                "DamagedRegenDelay",
            ],
        ),
        singles_component(
            "SCItemQuantumDriveParams",
            &["quantumFuelRequirement", "jumpRange", "disconnectRange"],
        )
        .field("params", class("QuantumDriveParams")),
        singles(
            "QuantumDriveParams",
            &[
                "driveSpeed",
                "cooldownTime",
                "spoolUpTime",
                "stageOneAccelRate",
                "stageTwoAccelRate",
                "engageSpeed",
            ],
        ),
        component("ItemResourceComponentParams").array("states", class("ItemResourceState")),
        TestStruct::new("ItemResourceState")
            .field("name", TestType::String)
            .field("signatureParams", class("SignatureParams"))
            .array("deltas", pointer("ItemResourceDelta")),
        TestStruct::new("SignatureParams")
            .field("EMSignature", class("Signature"))
            .field("IRSignature", class("Signature")),
        singles("Signature", &["nominalSignature", "decayRate"]),
        TestStruct::new("ItemResourceDelta"),
        TestStruct::new("ItemResourceDeltaGeneration")
            .parent("ItemResourceDelta")
            .field("generation", class("ItemResourceGeneration")),
        TestStruct::new("ItemResourceDeltaConversion")
            .parent("ItemResourceDelta")
            .field("consumption", class("ItemResourceGeneration"))
            .field("generation", class("ItemResourceGeneration")),
        TestStruct::new("ResourceUnit"),
        singles("SStandardResourceUnit", &["standardResourceUnits"]).parent("ResourceUnit"),
        TestStruct::new("ItemResourceGeneration")
            .field("resource", TestType::String)
            .field("resourceAmountPerSecond", pointer("ResourceUnit")),
        singles("SPowerSegmentResourceUnit", &["units"]).parent("ResourceUnit"),
    ]
}

fn others() -> Vec<TestStruct> {
    vec![
        component("SCItemMissileParams")
            .field("explosionParams", class("ExplosionParams"))
            .field("targetingParams", class("MissileTargetingParams"))
            .field("GCSParams", class("MissileGcsParams")),
        singles("MissileGcsParams", &["linearSpeed"]),
        singles(
            "MissileTargetingParams",
            &["lockTime", "lockingAngle", "lockRangeMin", "lockRangeMax"],
        )
        .field("trackingSignalType", TestType::String),
        singles_component("SHealthComponentParams", &["Health"]),
    ]
}

pub(super) fn builder() -> DataForgeBuilder {
    with_loop(
        TestStruct::new("Loop")
            .field("left", class("Loop"))
            .field("right", class("Loop"))
            .declared_size(0),
    )
}

pub(super) fn with_invalid_loop_layout() -> DataForgeBuilder {
    with_loop(
        TestStruct::new("Loop")
            .field("left", TestType::Int8)
            .field("right", TestType::Int8)
            .declared_size(3),
    )
}

fn with_loop(loop_shape: TestStruct) -> DataForgeBuilder {
    weapons(loop_shape)
        .into_iter()
        .chain(components())
        .chain(others())
        .fold(
            DataForgeBuilder::new().version(8).enumeration(
                "EItemType",
                [
                    "WeaponGun",
                    "Shield",
                    "QuantumDrive",
                    "PowerPlant",
                    "Cooler",
                    "Missile",
                    "Misc",
                    "Magazine",
                    "WeaponPersonal",
                ],
            ),
            DataForgeBuilder::structure,
        )
}
