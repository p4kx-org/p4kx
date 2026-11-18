// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with ships, a turret with ports, a docked vehicle, a leftover item and
//! vehicle implementations, for the loadouts of the catalog.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    TestPort, entity_schema,
};

pub(crate) const ALPHA: u128 = 0x10;
pub(crate) const SNUB: u128 = 0x11;
pub(crate) const BROKEN: u128 = 0x12;
pub(crate) const NESTED: u128 = 0x13;
pub(crate) const TURRET: u128 = 0x20;
pub(crate) const RACK: u128 = 0x21;
pub(crate) const GUN: u128 = 0x22;
pub(crate) const GUN_DEFAULT: u128 = 0x23;
pub(crate) const GUN_OVERRIDE: u128 = 0x24;
pub(crate) const GUN_TEMPLATE: u128 = 0x25;
pub(crate) const EMPTY_LOADOUT: u128 = 0x26;
pub(crate) const LOOP: u128 = 0x27;
pub(crate) const MISSING: u128 = 0x99;

const SHIPS: &str = "libs/foundry/records/entities/spaceships";
const ITEMS: &str = "libs/foundry/records/entities/scitem/ships";
const XML_DIR: &str = "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\";
const DEFINITION_DIR: &str = "Scripts/Entities/Vehicles/Implementations/Xml";

const ALPHA_XML: &str = r#"<Vehicle name="ACME_Alpha">
  <Parts>
    <Part name="hull" class="Animated">
      <Parts>
        <Part name="hardpoint_gun" class="ItemPort">
          <ItemPort minSize="1" maxSize="3" flags="$uneditable invisible" portTags="$Gun" requiredTags="Mount Unknown Dup" display_name="@LOC_EMPTY">
            <Types><Type type="WeaponGun"/></Types>
          </ItemPort>
        </Part>
        <Part name="Hardpoint_Gun" class="ItemPort">
          <ItemPort minSize="5" maxSize="6" flags="invisible"/>
        </Part>
        <Part name="hardpoint_turret" class="ItemPort">
          <ItemPort minSize="9" maxSize="9" flags="invisible" display_name="@fixture_xml_turret">
            <Types><Type type="VehicleTurret" subtypes="FixtureXmlOnly"/></Types>
          </ItemPort>
        </Part>
        <Part name="hardpoint_empty" class="ItemPort">
          <ItemPort minSize="2" maxSize="2" display_name="@fixture_vehicle_port">
            <Types>
              <Type type="Missile" subtypes="IR, EM, IR"/>
              <Type type="Bomb"/>
              <Type type="Missile" subtypes="Dumbfire, , Heat"/>
            </Types>
          </ItemPort>
        </Part>
        <Part name="hardpoint_empty" class="ItemPort">
          <ItemPort minSize="7" maxSize="8"/>
        </Part>
      </Parts>
    </Part>
  </Parts>
</Vehicle>"#;

fn definition(name: &str) -> String {
    format!("{DEFINITION_DIR}/{name}.xml")
}

fn ship(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("{SHIPS}/{}.xml", class.to_lowercase()))
}

fn item(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("{ITEMS}/{}.xml", class.to_lowercase()))
        .attach_type("WeaponGun")
}

fn gun_port(name: &str) -> TestPort {
    TestPort::new(name)
        .sizes(1, 2)
        .item_type("WeaponGun", ["Gun"])
        .default_item(GUN_DEFAULT)
}

fn alpha_loadout() -> TestLoadout {
    TestLoadout::Manual(vec![
        TestLoadoutEntry::class("hardpoint_turret", "ACME_Turret").loadout(TestLoadout::Manual(
            vec![TestLoadoutEntry::class("gun_1", "ACME_Gun_Override")],
        )),
        TestLoadoutEntry::class("hardpoint_gun", "ACME_Gun"),
        TestLoadoutEntry::class("hangar", "ACME_Snub"),
        TestLoadoutEntry::class("lost", "NoSuchClass"),
        TestLoadoutEntry::class("hardpoint_old", "ACME_Gun_template"),
        TestLoadoutEntry::reference("hardpoint_ref", MISSING),
    ])
}

fn entities() -> Vec<TestEntity> {
    vec![
        ship(ALPHA, "ACME_Alpha")
            .vehicle(definition("ACME_Alpha"), "")
            .port(
                TestPort::new("hardpoint_turret")
                    .sizes(2, 4)
                    .item_type("Turret", ["GunTurret"])
                    .flags("uneditable")
                    .tags("Turret", "Mount"),
            )
            .loadout(alpha_loadout()),
        ship(SNUB, "ACME_Snub")
            .attach_type("NOITEM_Vehicle_Snub")
            .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
                "snub_gun", "ACME_Gun",
            )])),
        ship(BROKEN, "ACME_Broken").vehicle(definition("ACME_Broken"), ""),
        ship(NESTED, "ACME_Nested").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::empty("body").loadout(TestLoadout::Manual(vec![
                TestLoadoutEntry::class("seat_1", "ACME_Gun"),
            ])),
        ])),
        item(LOOP, "ACME_Loop")
            .port(gun_port("loop_1"))
            .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                "loop_1", LOOP,
            )])),
        item(TURRET, "ACME_Turret")
            .port(gun_port("gun_1"))
            .port(gun_port("gun_2")),
        item(RACK, "ACME_Rack").port(
            TestPort::new("slot_1")
                .sizes(1, 1)
                .item_type("Missile", Vec::<String>::new()),
        ),
        item(GUN, "ACME_Gun").tags("Gun Unknown Dup mount"),
        item(GUN_DEFAULT, "ACME_Gun_Default"),
        item(GUN_OVERRIDE, "ACME_Gun_Override"),
        item(GUN_TEMPLATE, "ACME_Gun_template"),
        item(EMPTY_LOADOUT, "ACME_Empty_Loadout").loadout(TestLoadout::Manual(Vec::new())),
    ]
}

fn database() -> DataForgeBuilder {
    entities()
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        })
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
            format!("{XML_DIR}ACME_Broken.xml"),
            b"CryXmlB\0not really".to_vec(),
        ))
        .write_to(&path)?;
    Ok(path)
}
