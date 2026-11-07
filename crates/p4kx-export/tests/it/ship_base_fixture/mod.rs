// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with ships, cargo grids, a docked vehicle and vehicle implementations.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestCapacity, TestEntity, TestEntry, TestLoadout,
    TestLoadoutEntry, closed_container_record, entity_schema, open_container_record,
};

pub(crate) const ALPHA: u128 = 0x10;
pub(crate) const BETA: u128 = 0x11;
pub(crate) const GROUND: u128 = 0x12;
pub(crate) const BROKEN: u128 = 0x13;
const DUMMY: u128 = 0x14;
pub(crate) const TEMP: u128 = 0x1A;
pub(crate) const DISPLAY: u128 = 0x1B;
pub(crate) const GUN_DISPLAY: u128 = 0x27;
pub(crate) const MIXED: u128 = 0x15;
pub(crate) const UNKNOWN: u128 = 0x16;
pub(crate) const ROLLING: u128 = 0x17;
pub(crate) const CAR: u128 = 0x18;
pub(crate) const HUGE: u128 = 0x19;
const GRID_HUGE: u128 = 0x25;
const GRID_NEGATIVE: u128 = 0x26;
const CONTAINER_HUGE: u128 = 0x32;
const CONTAINER_NEGATIVE: u128 = 0x33;
const GRID_OPEN: u128 = 0x20;
const GRID_CLOSED: u128 = 0x21;
const GRID_TEMPLATE: u128 = 0x22;
const GUN: u128 = 0x23;
const SNUB: u128 = 0x24;
const CONTAINER_OPEN: u128 = 0x30;
const CONTAINER_CLOSED: u128 = 0x31;

const SHIPS: &str = "libs/foundry/records/entities/spaceships";
const GROUND_VEHICLES: &str = "libs/foundry/records/entities/groundvehicles";
const ITEMS: &str = "libs/foundry/records/entities/scitem/ships";
const XML_DIR: &str = "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\";
const DEFINITION_DIR: &str = "Scripts/Entities/Vehicles/Implementations/Xml";

pub(crate) const ALPHA_XML: &str = r#"<Vehicle name="ACME_Alpha">
  <Parts>
    <Part name="hull" class="Animated" mass="1000" damageMax="500">
      <DamageBehaviors>
        <DamageBehavior damageRatioMin="0.5"><Group name="Destroy"/></DamageBehavior>
      </DamageBehaviors>
      <Parts>
        <Part name="wing" id="wingId" class="Animated" mass="200" damageMax="100" detachRatio="0.25"/>
        <Part name="hardpoint_gun" class="ItemPort" mass="10" damageMax="40">
          <ItemPort flags="$uneditable"/>
        </Part>
      </Parts>
    </Part>
  </Parts>
  <Modifications>
    <Modification name="Heavy" patchFile="Modifications/ACME_Alpha_Heavy">
      <Elems><Elem idRef="wingId" name="damageMax" value="150"/></Elems>
    </Modification>
  </Modifications>
</Vehicle>"#;

const HEAVY: &str = r#"<Modifications>
  <Part id="wingId" name="wing" class="Animated" mass="260" damageMax="120"/>
</Modifications>"#;

fn definition(name: &str) -> String {
    format!("{DEFINITION_DIR}/{name}.xml")
}

fn ship(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("{SHIPS}/{}.xml", class.to_lowercase()))
}

fn ground(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(
        guid,
        class,
        format!("{GROUND_VEHICLES}/{}.xml", class.to_lowercase()),
    )
}

const ROLLER_XML: &str = r#"<Vehicle name="ACME_Roller">
  <Parts>
    <Part name="hull" class="Animated" mass="800" damageMax="300">
      <Parts>
        <Part name="wheel_left" class="SubPartWheel" mass="20"/>
        <Part name="wheel_right" class="SubPartWheel" mass="20"/>
      </Parts>
    </Part>
  </Parts>
</Vehicle>"#;

fn item(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("{ITEMS}/{}.xml", class.to_lowercase()))
}

fn loadout() -> TestLoadout {
    TestLoadout::Manual(vec![
        TestLoadoutEntry::class("grid_main", "ACME_Grid_Open"),
        TestLoadoutEntry::class("grid_aux", "ACME_Grid_Closed"),
        TestLoadoutEntry::class("grid_old", "ACME_Grid_template"),
        TestLoadoutEntry::class("gun", "ACME_Gun"),
        TestLoadoutEntry::class("hangar", "ACME_Snub"),
        TestLoadoutEntry::class("lost", "NoSuchClass"),
    ])
}

fn entities() -> Vec<TestEntity> {
    vec![
        ship(ALPHA, "ACME_Alpha")
            .size(3)
            .vehicle(definition("ACME_Alpha"), "")
            .loadout(loadout()),
        ship(BETA, "ACME_Beta")
            .size(4)
            .vehicle(definition("ACME_Alpha"), "Heavy")
            .loadout(loadout()),
        ground(GROUND, "ACME_Ground").physics_mass(5400.0),
        ship(ROLLING, "ACME_Rolling").vehicle(definition("ACME_Roller"), ""),
        ground(CAR, "ACME_Car").vehicle(definition("ACME_Roller"), ""),
        ship(BROKEN, "ACME_Broken").vehicle(definition("ACME_Broken"), ""),
        ship(HUGE, "ACME_Huge")
            .physics_mass(3000.0)
            .vehicle(definition("ACME_Huge"), ""),
        ship(DUMMY, "ACME_Dummy_Ship"),
        ship(TEMP, "ACME_Alpha_Temp").vehicle(definition("ACME_Alpha"), ""),
        ship(DISPLAY, "ACME_Beta_Display").vehicle(definition("ACME_Alpha"), "Heavy"),
        item(GUN_DISPLAY, "ACME_Gun_Display").attach_type("WeaponGun"),
        ship(MIXED, "ACME_Mixed").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("grid_huge", "ACME_Grid_Huge"),
            TestLoadoutEntry::class("grid_negative", "ACME_Grid_Negative"),
            TestLoadoutEntry::class("grid_ok", "ACME_Grid_Open"),
        ])),
        ship(UNKNOWN, "ACME_Unknown")
            .vehicle(definition("ACME_Flat"), "")
            .loadout(TestLoadout::Manual(vec![
                TestLoadoutEntry::class("grid_huge", "ACME_Grid_Huge"),
                TestLoadoutEntry::class("grid_negative", "ACME_Grid_Negative"),
            ])),
        item(GRID_HUGE, "ACME_Grid_Huge")
            .attach_type("CargoGrid")
            .container(CONTAINER_HUGE),
        item(GRID_NEGATIVE, "ACME_Grid_Negative")
            .attach_type("CargoGrid")
            .container(CONTAINER_NEGATIVE),
        item(GRID_OPEN, "ACME_Grid_Open")
            .attach_type("CargoGrid")
            .container(CONTAINER_OPEN),
        item(GRID_CLOSED, "ACME_Grid_Closed")
            .attach_type("CargoGrid")
            .container(CONTAINER_CLOSED),
        item(GRID_TEMPLATE, "ACME_Grid_template")
            .attach_type("CargoGrid")
            .container(CONTAINER_OPEN),
        item(GUN, "ACME_Gun").attach_type("WeaponGun"),
        item(SNUB, "ACME_Snub")
            .attach_type("NOITEM_Vehicle_Snub")
            .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
                "snub_grid",
                "ACME_Grid_Open",
            )])),
    ]
}

fn database() -> DataForgeBuilder {
    entities().iter().fold(
        entity_schema(DataForgeBuilder::new())
            .record(open_container_record(
                CONTAINER_OPEN,
                "grid_open",
                [2.5, 5.0, 2.5],
            ))
            .record(closed_container_record(
                CONTAINER_CLOSED,
                "grid_closed",
                TestCapacity::Centi(250.0),
            ))
            .record(open_container_record(
                CONTAINER_HUGE,
                "grid_huge",
                [1e30, 1e30, 1e30],
            ))
            .record(open_container_record(
                CONTAINER_NEGATIVE,
                "grid_negative",
                [-2.5, 5.0, -2.5],
            )),
        |builder, entity| builder.record(entity.record()),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACME_Alpha.xml"),
            ALPHA_XML.as_bytes().to_vec(),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}Modifications\\ACME_Alpha_Heavy.xml"),
            HEAVY.as_bytes().to_vec(),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACME_Roller.xml"),
            ROLLER_XML.as_bytes().to_vec(),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACME_Flat.xml"),
            br#"<Vehicle name="ACME_Flat"><Parts><Part name="hull" class="Animated" mass="10"/></Parts></Vehicle>"#.to_vec(),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACME_Huge.xml"),
            br#"<Vehicle name="ACME_Huge"><Parts><Part name="hull" class="Animated" mass="1e305" damageMax="1e305"/></Parts></Vehicle>"#.to_vec(),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACME_Broken.xml"),
            b"CryXmlB\0not really".to_vec(),
        ))
        .write_to(&path)?;
    Ok(path)
}

/// Writes an Archive whose DataForge is plain and whose vehicle file of `ACME_Alpha` is
/// encrypted, so that the ship base needs the Archive key for it.
pub(crate) fn write_encrypted_vehicle_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()))
        .entry(
            TestEntry::zstd(
                format!("{XML_DIR}ACME_Alpha.xml"),
                ALPHA_XML.as_bytes().to_vec(),
            )
            .encrypted(),
        )
        .write_to(&path)?;
    Ok(path)
}
