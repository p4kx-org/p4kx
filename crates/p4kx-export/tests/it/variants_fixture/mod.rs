// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive of ships that share vehicle definitions, Modifications and armour, and of
//! items that differ in one thing each.

pub(crate) mod item_rank;
mod items;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, GlobalIniBuilder,
    TestElement, TestEntry, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

pub(crate) const INVALID_COMPONENT_TYPE: u32 = u32::MAX - 1;
pub(crate) const ALPHA_XML: &str = "scripts/entities/vehicles/implementations/xml/acas_alpha.xml";
pub(crate) const OTHER_XML: &str = "scripts/entities/vehicles/implementations/xml/acas_other.xml";
const BROKEN_XML: &str = "scripts/entities/vehicles/implementations/xml/acas_broken.xml";
const XML_DIR: &str = "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\";

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn builder() -> DataForgeBuilder {
    [
        TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ),
        TestStruct::new("DataForgeComponentParams"),
        component("VehicleComponentParams")
            .field("vehicleDefinition", TestType::String)
            .field("modification", TestType::String)
            .field("vehicleName", TestType::Locale),
        component("SCItemVehicleArmorParams").field("damageMultiplier", TestType::Single),
    ]
    .into_iter()
    .chain(items::structs())
    .fold(
        DataForgeBuilder::new()
            .version(8)
            .enumeration("EItemType", ["PowerPlant"]),
        DataForgeBuilder::structure,
    )
}

const fn guid(index: u128) -> u128 {
    0x2000_0000_0000_4000_8000_0000_0000_0000 + index
}

struct Ship {
    class: &'static str,
    definition: &'static str,
    modification: &'static str,
    armour: Option<f32>,
}

const fn ship(
    class: &'static str,
    definition: &'static str,
    modification: &'static str,
    armour: Option<f32>,
) -> Ship {
    Ship {
        class,
        definition,
        modification,
        armour,
    }
}

const SHIPS: [Ship; 17] = [
    ship("ACAS_Alpha", ALPHA_XML, "", None),
    ship(
        "ACAS_Alpha_Case",
        "Scripts\\Entities\\Vehicles\\Implementations\\Xml\\ACAS_Alpha.xml",
        "",
        None,
    ),
    ship("ACAS_Alpha_Skin", ALPHA_XML, "Skin", None),
    ship("ACAS_Aim", ALPHA_XML, "Aim", None),
    ship("ACAS_Aim_Edition", ALPHA_XML, "Aim", None),
    ship("ACAS_Move", ALPHA_XML, "Move", None),
    ship("ACAS_Move_Edition", ALPHA_XML, "Move", None),
    ship("ACAS_Alpha_Edition", ALPHA_XML, "", None),
    ship("ACAS_Alpha_Zulu", ALPHA_XML, "", None),
    ship("ACAS_Beta", ALPHA_XML, "Beta", None),
    ship("ACAS_Beta_Edition", ALPHA_XML, "Beta", None),
    ship("ACAS_Alpha_Plated", ALPHA_XML, "", Some(0.5)),
    ship("ACAS_Other", OTHER_XML, "", None),
    ship("ACAS_Lonely", "", "", None),
    ship("ACAS_Broken", BROKEN_XML, "", None),
    ship(
        "ACAS_Missing",
        "scripts/entities/vehicles/implementations/xml/none.xml",
        "",
        None,
    ),
    ship("ACAS_Solo", OTHER_XML, "Beta", None),
];

fn record(index: usize, ship: &Ship) -> TestRecord {
    let mut components = vec![TestValue::Struct(
        TestInstance::new("VehicleComponentParams")
            .value("vehicleDefinition", TestValue::text(ship.definition))
            .value("modification", TestValue::text(ship.modification))
            .value(
                "vehicleName",
                TestValue::text(format!("@vehicle_Name{index}")),
            ),
    )];
    if let Some(multiplier) = ship.armour {
        components.push(TestValue::Struct(
            TestInstance::new("SCItemVehicleArmorParams")
                .value("damageMultiplier", TestValue::Single(multiplier)),
        ));
    }
    TestRecord::new(
        guid(index as u128 + 1),
        "EntityClassDefinition",
        format!("EntityClassDefinition.{}", ship.class),
        format!(
            "libs/foundry/records/entities/spaceships/{}.xml",
            ship.class
        ),
    )
    .value("Components", TestValue::Array(components))
}

fn port(tags: &str) -> TestElement {
    TestElement::new("Part")
        .attribute("class", "ItemPort")
        .child(
            TestElement::new("ItemPort")
                .attribute("id", "modPaint")
                .attribute("portTags", tags)
                .attribute("requiredTags", tags)
                .child(
                    TestElement::new("Types")
                        .child(TestElement::new("Type").attribute("type", "Paints")),
                )
                .child(
                    TestElement::new("Pitch")
                        .attribute("id", "pitch")
                        .attribute("speed", "1"),
                ),
        )
}

fn modification(name: &str, id_ref: &str, attribute: &str, value: &str) -> TestElement {
    TestElement::new("Modification")
        .attribute("name", name)
        .child(
            TestElement::new("Elems").child(
                TestElement::new("Elem")
                    .attribute("idRef", id_ref)
                    .attribute("name", attribute)
                    .attribute("value", value),
            ),
        )
}

fn vehicle_xml(tags: &str) -> Vec<u8> {
    TestElement::new("Vehicle")
        .child(
            TestElement::new("Parts")
                .child(
                    TestElement::new("Part")
                        .attribute("id", "hull")
                        .attribute("class", "Animated")
                        .attribute("material", "a.mtl"),
                )
                .child(port(tags)),
        )
        .child(
            TestElement::new("MovementParams")
                .attribute("id", "movement")
                .attribute("maxSpeed", "100"),
        )
        .child(
            TestElement::new("Modifications")
                .child(modification(
                    "Beta",
                    "modPaint",
                    "requiredTags",
                    "Paint_Beta",
                ))
                .child(modification("Skin", "hull", "material", "b.mtl"))
                .child(modification("Aim", "pitch", "speed", "2"))
                .child(modification("Move", "movement", "maxSpeed", "200")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

pub(crate) fn item_key_database(unreadable: bool) -> DataForgeBuilder {
    items::key_records(unreadable).fold(builder(), DataForgeBuilder::record)
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let database = SHIPS
        .iter()
        .enumerate()
        .fold(builder(), |database, (index, ship)| {
            database.record(record(index, ship))
        });
    let database = items::records().fold(database, DataForgeBuilder::record);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new().build(),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACAS_Alpha.xml"),
            vehicle_xml("Paint_Alpha"),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACAS_Other.xml"),
            vehicle_xml("Paint_Alpha"),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACAS_Broken.xml"),
            b"CryXmlB\0not really".to_vec(),
        ))
        .write_to(&path)?;
    Ok(path)
}

pub(crate) mod read_rows;
