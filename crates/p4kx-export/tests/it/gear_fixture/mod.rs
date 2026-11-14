// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with one item of each gear stat kind: armour, clothing, radar, tractor
//! beam, salvage modifier, a beam weapon and the mining laser, module and gadget.

mod mining;
mod structures;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestValue,
};

pub(crate) use mining::{GADGET, LASER, LASER_RANGE, LASER_TWIN, MODULE, MODULE_INSTABILITY};
use structures::database;

pub(crate) const ARMOR: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const ARMOR_DANGLING: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const CLOTHING: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const RADAR: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const TRACTOR: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const SALVAGE_MODIFIER: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const BEAM: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const PLAIN_MODIFIER: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const WRAPPED_TRACTOR: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const PARALLEL: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000a;
const RESISTANCE: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
const MISSING: u128 = 0x2000_0000_0000_4000_8000_0000_0000_00ff;

fn single(name: &str, value: f32) -> (String, TestValue) {
    (name.to_owned(), TestValue::Single(value))
}

fn instance(type_name: &str, values: Vec<(String, TestValue)>) -> TestValue {
    TestValue::Struct(
        values
            .into_iter()
            .fold(TestInstance::new(type_name), |instance, (name, value)| {
                instance.value(name, value)
            }),
    )
}

fn singles_of(type_name: &str, values: &[(&str, f32)]) -> TestValue {
    instance(
        type_name,
        values
            .iter()
            .map(|(name, value)| single(name, *value))
            .collect(),
    )
}

fn entity(guid: u128, class: &str, kind: &str, extra: Vec<TestValue>) -> TestRecord {
    let attach = instance(
        "SAttachableComponentParams",
        vec![(
            "AttachDef".to_owned(),
            instance(
                "SItemDefinition",
                vec![("Type".to_owned(), TestValue::text(kind))],
            ),
        )],
    );
    let mut components = vec![attach];
    components.extend(extra);
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/scitem/gear/{class}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn entry(multiplier: f32, threshold: f32) -> TestValue {
    singles_of(
        "DamageResistanceEntry",
        &[
            ("Multiplier", multiplier),
            ("Threshold", threshold),
            ("DamageCap", 0.0),
            ("DamageCapLimit", 0.0),
        ],
    )
}

fn resistance_record() -> TestRecord {
    TestRecord::new(
        RESISTANCE,
        "DamageResistanceMacro",
        "DamageResistanceMacro.HeavyArmor",
        "libs/foundry/records/damage/heavyarmor.xml",
    )
    .value(
        "damageResistance",
        instance(
            "DamageResistance",
            vec![
                ("IgnoreMeleeDamage".to_owned(), TestValue::Bool(false)),
                ("PhysicalResistance".to_owned(), entry(0.6, 0.0)),
                ("EnergyResistance".to_owned(), entry(0.75, 2.0)),
            ],
        ),
    )
}

fn armor(link: u128) -> TestValue {
    instance(
        "SCItemSuitArmorParams",
        vec![
            single("integrityMilestoneToBreak", 0.1),
            ("damageResistance".to_owned(), TestValue::Reference(link)),
        ],
    )
}

fn clothing() -> TestValue {
    instance(
        "SCItemClothingParams",
        vec![
            (
                "TemperatureResistance".to_owned(),
                singles_of(
                    "ClothingTemperature",
                    &[("MinResistance", -70.0), ("MaxResistance", 100.0)],
                ),
            ),
            (
                "RadiationResistance".to_owned(),
                singles_of(
                    "ClothingRadiation",
                    &[
                        ("MaximumRadiationCapacity", 26_800.0),
                        ("RadiationDissipationRate", 145.8),
                    ],
                ),
            ),
            (
                "Flight".to_owned(),
                singles_of("ClothingFlight", &[("gForceResistance", -0.5)]),
            ),
        ],
    )
}

fn radar() -> TestValue {
    instance(
        "SCItemRadarComponentParams",
        vec![
            (
                "aimAssist".to_owned(),
                singles_of(
                    "RadarAimAssist",
                    &[
                        ("distanceMinAssignment", 780.0),
                        ("distanceMaxAssignment", 1028.5),
                        ("outsideRangeBufferDistance", 60.0),
                    ],
                ),
            ),
            (
                "pingProperties".to_owned(),
                singles_of("RadarPing", &[("cooldownTime", 2.5)]),
            ),
        ],
    )
}

fn tractor() -> TestValue {
    instance(
        "SCItemWeaponComponentParams",
        vec![(
            "fireActions".to_owned(),
            TestValue::Array(vec![singles_of(
                "SWeaponActionFireTractorBeamParams",
                &[
                    ("minForce", 1500.0),
                    ("maxForce", 500_000.0),
                    ("minDistance", 0.0),
                    ("maxDistance", 150.0),
                    ("fullStrengthDistance", 75.0),
                    ("maxVolume", 300_000.0),
                    ("maxAngle", 60.0),
                ],
            )]),
        )],
    )
}

fn wrapped_tractor() -> TestValue {
    instance(
        "SCItemWeaponComponentParams",
        vec![(
            "fireActions".to_owned(),
            TestValue::Array(vec![
                instance("SWeaponActionFireBeamParams", Vec::new()),
                instance(
                    "SWeaponActionToggleParams",
                    vec![(
                        "weaponAction".to_owned(),
                        singles_of(
                            "SWeaponActionFireTractorBeamParams",
                            &[("maxForce", 42.0), ("maxDistance", 7.0)],
                        ),
                    )],
                ),
            ]),
        )],
    )
}

fn parallel() -> TestValue {
    let beam = || {
        instance(
            "SWeaponActionFireBeamParams",
            vec![(
                "damagePerSecond".to_owned(),
                singles_of("DamageInfo", &[("DamageEnergy", 10.0)]),
            )],
        )
    };
    instance(
        "SCItemWeaponComponentParams",
        vec![(
            "fireActions".to_owned(),
            TestValue::Array(vec![instance(
                "SWeaponActionParallelParams",
                vec![(
                    "weaponActions".to_owned(),
                    TestValue::Array(vec![beam(), beam()]),
                )],
            )]),
        )],
    )
}

fn beam() -> TestValue {
    instance(
        "SCItemWeaponComponentParams",
        vec![(
            "fireActions".to_owned(),
            TestValue::Array(vec![instance(
                "SWeaponActionFireBeamParams",
                vec![(
                    "damagePerSecond".to_owned(),
                    singles_of(
                        "DamageInfo",
                        &[("DamagePhysical", 0.0), ("DamageEnergy", 2340.0)],
                    ),
                )],
            )]),
        )],
    )
}

fn modifiers(salvage: bool) -> TestValue {
    let mut stats = vec![single("fireRateMultiplier", 1.0)];
    if salvage {
        stats.push((
            "salvageModifier".to_owned(),
            singles_of(
                "SSalvageModifier",
                &[
                    ("salvageSpeedMultiplier", 0.15),
                    ("radiusMultiplier", 3.5),
                    ("extractionEfficiency", 0.9),
                ],
            ),
        ));
        stats.push((
            "tractorModifier".to_owned(),
            singles_of(
                "STractorModifier",
                &[
                    ("forceMultiplier", 1.0),
                    ("fullStrengthDistMultiplier", 1.0),
                    ("maxDistMultiplier", 1.0),
                    ("maxVolumeMultiplier", 1.0),
                ],
            ),
        ));
    }
    instance(
        "EntityComponentAttachableModifierParams",
        vec![(
            "modifiers".to_owned(),
            TestValue::Array(vec![instance(
                "ItemWeaponModifiersParams",
                vec![(
                    "weaponModifier".to_owned(),
                    instance(
                        "SWeaponModifierParams",
                        vec![("weaponStats".to_owned(), instance("SWeaponStats", stats))],
                    ),
                )],
            )]),
        )],
    )
}

fn records() -> Vec<TestRecord> {
    let mut records = vec![
        resistance_record(),
        entity(ARMOR, "armor", "Char_Armor_Torso", vec![armor(RESISTANCE)]),
        entity(
            ARMOR_DANGLING,
            "armor_dangling",
            "Char_Armor_Torso",
            vec![armor(MISSING)],
        ),
        entity(CLOTHING, "clothing", "Char_Clothing_Hat", vec![clothing()]),
        entity(RADAR, "radar", "Radar", vec![radar()]),
        entity(TRACTOR, "tractor", "TractorBeam", vec![tractor()]),
        entity(BEAM, "beam", "WeaponGun", vec![beam()]),
        entity(PARALLEL, "parallel", "WeaponGun", vec![parallel()]),
        entity(
            WRAPPED_TRACTOR,
            "wrapped_tractor",
            "TractorBeam",
            vec![wrapped_tractor()],
        ),
        entity(
            SALVAGE_MODIFIER,
            "salvage_modifier",
            "SalvageModifier",
            vec![modifiers(true)],
        ),
        entity(
            PLAIN_MODIFIER,
            "plain_modifier",
            "SalvageModifier",
            vec![modifiers(false)],
        ),
    ];
    records.extend(mining::records());
    records
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let database = records()
        .into_iter()
        .fold(database(), DataForgeBuilder::record);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new().build(),
        ))
        .write_to(&path)?;
    Ok(path)
}
