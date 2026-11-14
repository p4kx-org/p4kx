// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestRecord, TestValue};

use super::values::{damage, entity, instance, single};
use super::{AMMO, MAGAZINE};

fn launch(pellets: i32, cost: i32) -> TestValue {
    instance(
        "SProjectileLauncher",
        vec![
            ("pelletCount".to_owned(), TestValue::Int32(pellets)),
            ("pellets".to_owned(), TestValue::Int32(9)),
            ("ammoCost".to_owned(), TestValue::Int32(cost)),
            (
                "spreadParams".to_owned(),
                instance(
                    "SSpreadParams",
                    vec![
                        single("min", 0.18),
                        single("max", 5.0),
                        single("firstAttack", 1.35),
                        single("attack", 1.25),
                        single("decay", 6.5),
                    ],
                ),
            ),
        ],
    )
}

fn rapid(rate: i32) -> TestValue {
    instance(
        "SWeaponActionFireRapidParams",
        vec![
            ("name".to_owned(), TestValue::text("rapid")),
            ("fireRate".to_owned(), TestValue::Int32(rate)),
            single("heatPerShot", 0.5),
            ("launchParams".to_owned(), launch(1, 1)),
        ],
    )
}

fn charged() -> TestValue {
    instance(
        "SWeaponActionFireChargedParams",
        vec![
            single("chargeTime", 1.5),
            single("overchargeTime", 3.0),
            single("cooldownTime", 0.25),
            ("weaponAction".to_owned(), rapid(120)),
        ],
    )
}

pub(super) fn weapon_component(magazine: Option<u128>) -> TestValue {
    let mut fields = vec![(
        "fireActions".to_owned(),
        TestValue::Array(vec![rapid(600), charged()]),
    )];
    if let Some(magazine) = magazine {
        fields.push((
            "ammoContainerRecord".to_owned(),
            TestValue::Reference(magazine),
        ));
    }
    instance("SCItemWeaponComponentParams", fields)
}

pub(super) fn container(ammo: u128, capacity: i32) -> TestValue {
    instance(
        "SAmmoContainerComponentParams",
        vec![
            ("ammoParamsRecord".to_owned(), TestValue::Reference(ammo)),
            ("maxAmmoCount".to_owned(), TestValue::Int32(capacity)),
        ],
    )
}

pub(super) fn gun(guid: u128, class: &str, ammo: u128) -> TestRecord {
    entity(
        guid,
        class,
        "WeaponGun",
        vec![weapon_component(None), container(ammo, 200)],
    )
}

pub(super) fn personal_gun(guid: u128) -> TestRecord {
    entity(
        guid,
        "personal_gun",
        "WeaponPersonal",
        vec![weapon_component(Some(MAGAZINE))],
    )
}

pub(super) fn magazine(guid: u128) -> TestRecord {
    entity(guid, "magazine", "Magazine", vec![container(AMMO, 30)])
}

pub(super) fn ammo(guid: u128) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        "AmmoParams.Ammo_Test",
        "libs/foundry/records/ammoparams/vehicle/ammo_test.xml",
    )
    .value("speed", TestValue::Single(1100.0))
    .value("lifetime", TestValue::Single(2.5))
    .value(
        "projectileParams",
        instance(
            "BulletProjectileParams",
            vec![
                ("damage".to_owned(), damage(30.0, 2.0)),
                (
                    "detonationParams".to_owned(),
                    instance(
                        "ProjectileDetonationParams",
                        vec![(
                            "explosionParams".to_owned(),
                            instance(
                                "ExplosionParams",
                                vec![
                                    single("minRadius", 1.0),
                                    single("maxRadius", 4.0),
                                    ("damage".to_owned(), damage(7.0, 0.0)),
                                ],
                            ),
                        )],
                    ),
                ),
            ],
        ),
    )
}

pub(super) fn cyclic_ammo(guid: u128) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        "AmmoParams.Ammo_Unbuildable",
        "libs/foundry/records/ammoparams/vehicle/ammo_unbuildable.xml",
    )
    .value("trap", TestValue::Struct(TestInstance::new("Loop")))
}
