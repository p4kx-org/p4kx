// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with one item of each stat kind and an item without stats.

mod components;
mod structures;
mod values;
mod weapons;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry};

use components::{cooler_resources, drive, health, missile, plant_resources, shield};
use values::entity;
use weapons::{ammo, container, cyclic_ammo, gun, magazine, personal_gun, weapon_component};

pub(crate) const GUN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SHIELD: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const DRIVE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const PLANT: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const COOLER: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const MISSILE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const PLAIN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const DANGLING_GUN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const PERSONAL_GUN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const WRONG_TYPE_GUN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000a;
pub(crate) const UNBOUNDED_DRIVE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000b;
pub(crate) const CYCLIC_AMMO_GUN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000c;
const MAGAZINE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_00a0;
const AMMO: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
const CYCLIC_AMMO: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0002;
const MISSING_AMMO: u128 = 0x2000_0000_0000_4000_8000_0000_0000_00ff;

fn database(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(ammo(AMMO))
        .record(cyclic_ammo(CYCLIC_AMMO))
        .record(gun(CYCLIC_AMMO_GUN, "unbuildable_ammo_gun", CYCLIC_AMMO))
        .record(magazine(MAGAZINE))
        .record(gun(GUN, "gun", AMMO))
        .record(gun(DANGLING_GUN, "dangling_gun", MISSING_AMMO))
        .record(personal_gun(PERSONAL_GUN))
        .record(entity(
            WRONG_TYPE_GUN,
            "wrong_type_gun",
            "WeaponGun",
            vec![weapon_component(None), container(MAGAZINE, 200)],
        ))
        .record(entity(
            SHIELD,
            "shield",
            "Shield",
            vec![shield(8_000.0), health()],
        ))
        .record(entity(DRIVE, "drive", "QuantumDrive", vec![drive(5.0e10)]))
        .record(entity(
            UNBOUNDED_DRIVE,
            "unbounded_drive",
            "QuantumDrive",
            vec![drive(f32::MAX)],
        ))
        .record(entity(
            PLANT,
            "plant",
            "PowerPlant",
            vec![plant_resources(12.0)],
        ))
        .record(entity(
            COOLER,
            "cooler",
            "Cooler",
            vec![cooler_resources(40.0)],
        ))
        .record(entity(MISSILE, "missile", "Missile", vec![missile()]))
        .record(entity(PLAIN, "plain", "Misc", vec![health()]))
}

fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new().build(),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_with_builder(dir, &database(structures::builder()))
}

/// Writes the fixture with a reachable ammo layout whose declared size is inconsistent.
pub(crate) fn write_archive_with_invalid_ammo(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_with_builder(dir, &database(structures::with_invalid_loop_layout()))
}

pub(crate) fn write_shield_variant_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let builder = structures::builder()
        .record(entity(
            SHIELD,
            "shield_base",
            "Shield",
            vec![shield(8_000.0)],
        ))
        .record(entity(
            SHIELD + 1,
            "shield_base_twin",
            "Shield",
            vec![shield(8_000.0)],
        ))
        .record(entity(
            SHIELD + 2,
            "shield_different",
            "Shield",
            vec![shield(9_000.0)],
        ))
        .record(entity(
            SHIELD + 3,
            "shield_with_resources",
            "Shield",
            vec![shield(8_000.0), plant_resources(12.0)],
        ));
    write_archive_with_builder(dir, &builder)
}

fn write_archive_with_builder(
    dir: &Path,
    builder: &DataForgeBuilder,
) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", builder.build()).encrypted())
        .entry(english())
        .write_to(&path)?;
    Ok(path)
}
