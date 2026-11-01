// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A Loadout entry that names an Entity both by Reference and by class name attaches the Entity of
//! the Reference, and any entry beats the default item of its port.

use std::error::Error;

use p4kx_assembly::{Attachment, LoadoutResolver};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, TestPort, entity_schema,
};

type TestResult = Result<(), Box<dyn Error>>;

const SHIP: u128 = 0x1000;
const GIMBAL: u128 = 0x2000;
const WEAPON: u128 = 0x3000;

fn ship(entry: TestLoadoutEntry) -> TestEntity {
    TestEntity::new(SHIP, "TEST_Ship", "entities/test_ship.xml")
        .loadout(TestLoadout::Manual(vec![entry]))
}

/// Returns the class name of the Entity on the first port of the ship.
fn attached_class(ship: &TestEntity) -> Result<String, Box<dyn Error>> {
    let bytes = [
        ship,
        &TestEntity::new(GIMBAL, "TEST_Gimbal", "entities/test_gimbal.xml"),
        &TestEntity::new(WEAPON, "TEST_Weapon", "entities/test_weapon.xml"),
    ]
    .into_iter()
    .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
        builder.record(entity.record())
    })
    .build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let entity = resolver.entities().find("TEST_Ship")?;
    let tree = resolver.resolve(&entity)?;
    let port = tree.ports().first().ok_or("no port")?;
    match port.attachment() {
        Attachment::Entity(child) => Ok(child.class_name().to_owned()),
        other => Err(format!("{other:?}").into()),
    }
}

#[test]
fn the_reference_wins_over_a_class_name_that_disagrees() -> TestResult {
    let entry = TestLoadoutEntry::reference("mount", GIMBAL).named("TEST_Weapon");
    assert_eq!(attached_class(&ship(entry))?, "TEST_Gimbal");
    Ok(())
}

#[test]
fn the_class_name_is_used_without_a_reference() -> TestResult {
    let entry = TestLoadoutEntry::class("mount", "TEST_Weapon");
    assert_eq!(attached_class(&ship(entry))?, "TEST_Weapon");
    Ok(())
}

#[test]
fn an_entry_beats_the_default_item_of_its_port() -> TestResult {
    let entry = TestLoadoutEntry::reference("mount", GIMBAL);
    let ship = ship(entry).port(TestPort::new("mount").default_item(WEAPON));
    assert_eq!(attached_class(&ship)?, "TEST_Gimbal");
    Ok(())
}
