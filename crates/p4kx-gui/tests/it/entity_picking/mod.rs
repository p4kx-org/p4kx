// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the Entities tab tests: a synthetic Archive with a ship, a vehicle, weapons and a
//! shield, opened in a headless window.

use std::error::Error;

use egui_kittest::Harness;
use p4kx_gui::{EntitiesStatus, Language, Model, WorkTab};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestPort,
    assembly_entries, entity_schema,
};
use tempfile::TempDir;

use crate::tagging::{archive, harness_sized, wait_until};
use crate::waiting::wait_for;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;
pub(crate) type Window = (TempDir, Harness<'static, Model>);

const FOLDER: &str = "libs/foundry/records/entities";

fn gun(guid: u128, class: &str, size: i32) -> TestEntity {
    TestEntity::new(
        guid,
        class,
        format!("{FOLDER}/scitem/ships/weapons/{}.xml", class.to_lowercase()),
    )
    .attach_type("WeaponGun")
    .sub_type("Gun")
    .size(size)
    .geometry("Objects/Test/gun.cgf", None)
}

fn entities() -> Vec<TestEntity> {
    let ship = TestEntity::new(
        0x1,
        "TEST_Ship",
        format!("{FOLDER}/spaceships/test_ship.xml"),
    )
    .geometry("Objects\\Test\\ship.cgf", None)
    .port(
        TestPort::new("hardpoint_weapon_nose")
            .helper("hp_nose", [0.0, 0.0, 1.0], [0.0; 3])
            .sizes(1, 3)
            .item_type("WeaponGun", ["Gun"]),
    )
    .port(TestPort::new("hardpoint_relay").helper("hardpoint_relay", [0.0; 3], [0.0; 3]))
    .loadout(TestLoadout::Manual(vec![
        TestLoadoutEntry::class("hardpoint_weapon_nose", "TEST_Gun_S2"),
        TestLoadoutEntry::class("hardpoint_relay", "TEST_Relay"),
    ]));
    let cart = TestEntity::new(
        0x2,
        "TEST_Cart",
        format!("{FOLDER}/groundvehicles/test_cart.xml"),
    );
    let relay = TestEntity::new(
        0x3,
        "TEST_Relay",
        format!("{FOLDER}/scitem/ships/relays/test_relay.xml"),
    )
    .attach_type("Relay")
    .geometry("Objects/Test/gun.cgf", None);
    let shield = TestEntity::new(
        0x4,
        "TEST_Shield",
        format!("{FOLDER}/scitem/ships/shields/test_shield.xml"),
    )
    .attach_type("Shield")
    .size(2);
    vec![
        ship,
        cart,
        relay,
        shield,
        gun(0x10, "TEST_Gun_S1", 1),
        gun(0x11, "TEST_Gun_S2", 2),
        gun(0x12, "TEST_Gun_S3", 3),
        gun(0x13, "TEST_Gun_S4", 4),
    ]
}

/// Returns the Entries of the synthetic Archive.
pub(crate) fn entries() -> Vec<TestEntry> {
    let database = entities()
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        })
        .build();
    let mut entries = vec![TestEntry::zstd("Data\\Game2.dcb", database)];
    entries.extend(assembly_entries().into_iter().skip(1));
    entries
}

pub(crate) fn listed(model: &Model) -> bool {
    matches!(
        model.entities().status(),
        EntitiesStatus::Ready(_) | EntitiesStatus::Failed(_) | EntitiesStatus::NoEntities
    )
}

/// Returns the class names of the Entities the picker shows.
pub(crate) fn shown(model: &Model) -> Vec<String> {
    match model.entities().status() {
        EntitiesStatus::Ready(list) => (0..list.shown_len())
            .filter_map(|at| list.get(at))
            .map(|entity| entity.class_name().to_owned())
            .collect(),
        _ => Vec::new(),
    }
}

/// Returns the ports of the Loadout as `(path, change)` once it is resolved.
pub(crate) fn ports(model: &Model) -> Vec<(String, p4kx_gui::RowChange)> {
    model
        .entities()
        .loadout()
        .and_then(p4kx_gui::LoadoutSession::view)
        .map_or_default(|view| {
            view.rows()
                .iter()
                .map(|row| (row.path().to_owned(), row.change()))
                .collect()
        })
}

pub(crate) fn resolved(model: &Model) -> bool {
    model
        .entities()
        .loadout()
        .is_some_and(|session| session.view().is_some() && !session.is_resolving())
}

/// Opens the Archive of `entries` in a window of `size` on the Entities tab and waits until the
/// Entities are listed.
pub(crate) fn opened_sized(
    entries: Vec<TestEntry>,
    language: Language,
    size: [f32; 2],
) -> Result<Window, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = archive(dir.path(), entries)?;
    let harness = opened_from(&path, language, size)?;
    Ok((dir, harness))
}

fn opened_from(
    path: &std::path::Path,
    language: Language,
    size: [f32; 2],
) -> Result<Harness<'static, Model>, Box<dyn Error>> {
    let mut harness = harness_sized(Model::new(language), size);
    harness.state_mut().open(path.to_path_buf(), || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    harness.state_mut().set_work_tab(WorkTab::Entities);
    harness.run_steps(2);
    wait_until(&mut harness, listed)?;
    Ok(harness)
}

/// Opens the Archive in the channel folder `StarCitizen/LIVE` of a temporary folder that holds
/// a `Bin64` folder, so that the folder counts as a Star Citizen installation; returns the
/// window and the channel folder.
pub(crate) fn opened_in_installation(
    language: Language,
) -> Result<(Window, std::path::PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let channel = dir.path().join("StarCitizen").join("LIVE");
    std::fs::create_dir_all(channel.join("Bin64"))?;
    let path = archive(&channel, entries())?;
    let harness = opened_from(&path, language, [1400.0, 900.0])?;
    Ok(((dir, harness), channel))
}

pub(crate) fn opened(language: Language) -> Result<Window, Box<dyn Error>> {
    opened_sized(entries(), language, [1400.0, 900.0])
}

/// Selects the Entity `name` and waits until its Loadout is resolved.
pub(crate) fn select(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    let at = shown(harness.state())
        .iter()
        .position(|shown| shown == name)
        .ok_or("the Entity is not listed")?;
    harness.state_mut().select_entity(at, || {});
    wait_until(harness, resolved)?;
    wait_until(harness, |model| {
        !model
            .entities()
            .loadout()
            .is_some_and(p4kx_gui::LoadoutSession::is_resolving)
    })
}

/// Waits until the Loadout is resolved again after a change.
pub(crate) fn settle(harness: &mut Harness<'_, Model>) -> TestResult {
    wait_for(harness, "the Loadout to settle", |harness| {
        resolved(harness.state())
    })?;
    harness.run_steps(3);
    Ok(())
}
