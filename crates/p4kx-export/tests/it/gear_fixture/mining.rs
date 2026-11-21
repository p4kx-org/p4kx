// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The mining items of the gear fixture: a laser, an active module and a gadget.

use p4kx_testkit::{TestRecord, TestValue};

use super::{entity, instance, single, singles_of};

pub(crate) const LASER: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0010;
pub(crate) const MODULE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0011;
pub(crate) const GADGET: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0012;
/// A laser equal to `LASER` in every compared field.
pub(crate) const LASER_TWIN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0013;
/// A laser that differs from `LASER` only in the range of its beams.
pub(crate) const LASER_RANGE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0014;
/// A module that differs from `MODULE` only in the laser instability it applies.
pub(crate) const MODULE_INSTABILITY: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0015;
pub(crate) const GLOBALS: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0002;

const MODIFIERS: [&str; 7] = [
    "laserInstability",
    "optimalChargeWindowSizeModifier",
    "resistanceModifier",
    "shatterdamageModifier",
    "clusterFactorModifier",
    "optimalChargeWindowRateModifier",
    "catastrophicChargeWindowRateModifier",
];

fn float_modifier(value: f32) -> TestValue {
    singles_of("FloatModifierMultiplicative", &[("value", value)])
}

/// A `MiningLaserModifiers` struct that sets the modifiers named in `set` and leaves the rest null.
fn laser_modifiers(set: &[(&str, f32)]) -> TestValue {
    let mut values = vec![("isOptimalRateGood".to_owned(), TestValue::Bool(true))];
    values.extend(MODIFIERS.iter().map(|name| {
        let value = set
            .iter()
            .find(|(candidate, _)| candidate == name)
            .map_or(TestValue::Null, |(_, value)| float_modifier(*value));
        ((*name).to_owned(), value)
    }));
    instance("MiningLaserModifiers", values)
}

fn filter(value: Option<f32>) -> TestValue {
    instance(
        "MiningFilterParams",
        vec![(
            "filterModifier".to_owned(),
            value.map_or(TestValue::Null, float_modifier),
        )],
    )
}

fn lifetime(seconds: Option<f32>) -> (String, TestValue) {
    (
        "modifierLifetime".to_owned(),
        seconds.map_or(TestValue::Null, |seconds| {
            singles_of("ItemModifierTimedLife", &[("lifetime", seconds)])
        }),
    )
}

fn beam(hit_type: &str, damage: f32, range: f32) -> TestValue {
    instance(
        "SWeaponActionFireBeamParams",
        vec![
            ("hitType".to_owned(), TestValue::text(hit_type)),
            single("fullDamageRange", range),
            single("zeroDamageRange", range + 5.0),
            (
                "damagePerSecond".to_owned(),
                singles_of("DamageInfo", &[("DamageEnergy", damage)]),
            ),
        ],
    )
}

fn laser(range: f32) -> Vec<TestValue> {
    vec![
        instance(
            "SCItemWeaponComponentParams",
            vec![(
                "fireActions".to_owned(),
                TestValue::Array(vec![
                    beam("ElectricArc", 3900.0, range),
                    beam("Extraction", 1295.0, range),
                ]),
            )],
        ),
        instance(
            "SEntityComponentMiningLaserParams",
            vec![
                single("throttleLerpSpeed", 7.0),
                single("throttleMinimum", 0.2),
                ("usesPowerThrottle".to_owned(), TestValue::Bool(false)),
                ("globalParams".to_owned(), TestValue::Reference(GLOBALS)),
                (
                    "miningLaserModifiers".to_owned(),
                    laser_modifiers(&[
                        ("laserInstability", 35.0),
                        ("optimalChargeWindowSizeModifier", 40.0),
                        ("resistanceModifier", 25.0),
                        ("optimalChargeWindowRateModifier", -40.0),
                    ]),
                ),
                ("filterParams".to_owned(), filter(Some(40.0))),
            ],
        ),
    ]
}

fn attachable(method: &str, charges: i32, modifiers: Vec<TestValue>) -> TestValue {
    instance(
        "EntityComponentAttachableModifierParams",
        vec![
            ("activationMethod".to_owned(), TestValue::text(method)),
            ("charges".to_owned(), TestValue::Int32(charges)),
            ("canInterrupt".to_owned(), TestValue::Bool(true)),
            ("isInterruptible".to_owned(), TestValue::Bool(false)),
            ("modifiers".to_owned(), TestValue::Array(modifiers)),
        ],
    )
}

fn damage_modifier(index: i32, multiplier: f32) -> TestValue {
    instance(
        "ItemWeaponModifiersParams",
        vec![
            ("fireActionIndex".to_owned(), TestValue::Int32(index)),
            lifetime(Some(30.0)),
            (
                "weaponModifier".to_owned(),
                instance(
                    "SWeaponModifierParams",
                    vec![(
                        "weaponStats".to_owned(),
                        instance("SWeaponStats", vec![single("damageMultiplier", multiplier)]),
                    )],
                ),
            ),
        ],
    )
}

fn module(instability: f32) -> TestValue {
    attachable(
        "ActivateOnDemand",
        6,
        vec![
            damage_modifier(0, 1.35),
            damage_modifier(1, 0.85),
            instance(
                "ItemMiningModifierParams",
                vec![
                    lifetime(Some(30.0)),
                    (
                        "MiningLaserModifier".to_owned(),
                        laser_modifiers(&[
                            ("laserInstability", instability),
                            ("shatterdamageModifier", -10.0),
                        ]),
                    ),
                ],
            ),
            instance(
                "MiningFilterItemModifierParams",
                vec![
                    lifetime(Some(30.0)),
                    ("filterParams".to_owned(), filter(None)),
                ],
            ),
            singles_of("ItemMiningBoosterParams", &[("powerLevelChange", 2.0)]),
        ],
    )
}

fn gadget() -> TestValue {
    attachable(
        "ActivateOnDemand",
        -1,
        vec![instance(
            "ItemMineableRockModifierParams",
            vec![
                lifetime(None),
                (
                    "MiningLaserModifier".to_owned(),
                    laser_modifiers(&[
                        ("optimalChargeWindowSizeModifier", -30.0),
                        ("resistanceModifier", -25.0),
                        ("clusterFactorModifier", 60.0),
                    ]),
                ),
            ],
        )],
    )
}

pub(super) fn records() -> Vec<TestRecord> {
    vec![
        entity(LASER, "mining_laser", "WeaponMining", laser(40.0)),
        entity(LASER_TWIN, "mining_laser_twin", "WeaponMining", laser(40.0)),
        entity(
            LASER_RANGE,
            "mining_laser_range",
            "WeaponMining",
            laser(60.0),
        ),
        entity(
            MODULE,
            "mining_module",
            "MiningModifier",
            vec![module(-10.0)],
        ),
        entity(
            MODULE_INSTABILITY,
            "mining_module_instability",
            "MiningModifier",
            vec![module(-12.0)],
        ),
        entity(GADGET, "mining_gadget", "MiningModifier", vec![gadget()]),
        TestRecord::new(
            GLOBALS,
            "MiningLaserGlobalParams",
            "MiningLaserGlobalParams.Test",
            "libs/foundry/records/mining/test_laser_globals.xml",
        )
        .value("throttleChangePerAction", TestValue::Single(0.25))
        .value("blockThrottleChangeWhenNotFiring", TestValue::Bool(true))
        .value(
            "throttleRTPC",
            instance(
                "AudioRtpc",
                vec![("rtpc".to_owned(), TestValue::text("FixtureThrottle"))],
            ),
        ),
    ]
}
