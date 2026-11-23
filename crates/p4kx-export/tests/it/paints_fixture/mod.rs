// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with paints, ships with paint ports and a tag tree.
pub(crate) mod vehicle_lookup;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, GlobalIniBuilder,
    TestElement, TestEntry, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

pub(crate) const PAINT_ALPHA_RED: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PAINT_ALPHA_VARIANT: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const PAINT_BETA: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const PAINT_GAMMA: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const PAINT_ORPHAN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const PAINT_UPPER: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const PAINT_DELTA_MIL: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const PAINT_DELTA_PORT: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const SHIP_DELTA: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const SHIP_EPSILON: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const PAINT_ZETA: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const PAINT_ZETA_REQ: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000a;
pub(crate) const PAINT_ZETA_CASE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000b;
pub(crate) const PAINT_ZETA_TWO: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000c;
pub(crate) const PAINT_ETA_TREE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000d;
pub(crate) const SHIP_ZETA: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const SHIP_ETA: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const SHIP_ALPHA: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SHIP_BETA: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const SHIP_GAMMA: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const SHIP_BROKEN: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const SHIP_MISSING: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0005;

const PLANT: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PAINT_TEST: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0002;
const DATABASE: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0001;
const TAG_FAMILY: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0002;
const TAG_VARIANT: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0003;
const TAG_PATH: &str = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";
const ALPHA_XML: &str = "scripts/entities/vehicles/implementations/xml/acas_alpha.xml";
const XML_DIR: &str = "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\";

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn builder() -> DataForgeBuilder {
    [
        TestStruct::new("SCItemLocalization")
            .field("Name", TestType::Locale)
            .field("Description", TestType::Locale),
        TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ),
        TestStruct::new("DataForgeComponentParams"),
        component("SAttachableComponentParams").field("AttachDef", class("SItemDefinition")),
        TestStruct::new("SItemDefinition")
            .field("Type", TestType::Enum("EItemType".to_owned()))
            .field("Tags", TestType::String)
            .field("RequiredTags", TestType::String)
            .field("Localization", class("SCItemLocalization")),
        component("VehicleComponentParams")
            .field("vehicleDefinition", TestType::String)
            .field("modification", TestType::String)
            .field("vehicleName", TestType::Locale),
        component("SItemPortContainerComponentParams").array("Ports", class("SItemPortDef")),
        TestStruct::new("SItemPortDef")
            .field("Name", TestType::String)
            .field("PortTags", TestType::String)
            .field("RequiredPortTags", TestType::String)
            .array("Types", class("SItemPortDefTypes")),
        TestStruct::new("SItemPortDefTypes").field("Type", TestType::String),
        TestStruct::new("Tag")
            .field("tagName", TestType::String)
            .field("legacyGUID", TestType::UInt32)
            .array("children", TestType::ReferenceTo("Tag".into())),
        TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".into())),
    ]
    .into_iter()
    .fold(
        DataForgeBuilder::new()
            .version(8)
            .enumeration("EItemType", ["PowerPlant", "Paints"]),
        DataForgeBuilder::structure,
    )
}

fn references(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().map(|g| TestValue::Reference(*g)).collect())
}

fn tag(guid: u128, name: &str, children: &[u128]) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), TAG_PATH)
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(u32::MAX))
        .value("children", references(children))
}

fn entity(guid: u128, class: &str, path: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/{path}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn attach(kind: &str, tags: &str, required: &str, name: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SAttachableComponentParams").value(
            "AttachDef",
            TestValue::Struct(
                TestInstance::new("SItemDefinition")
                    .value("Type", TestValue::text(kind))
                    .value("Tags", TestValue::text(tags))
                    .value("RequiredTags", TestValue::text(required))
                    .value(
                        "Localization",
                        TestValue::Struct(
                            TestInstance::new("SCItemLocalization")
                                .value("Name", TestValue::text(name))
                                .value("Description", TestValue::text("@paint_Desc")),
                        ),
                    ),
            ),
        ),
    )
}

fn paint(guid: u128, class: &str, tags: &str, required: &str) -> TestRecord {
    entity(
        guid,
        class,
        &format!("scitem/ships/paints/{class}"),
        vec![attach(
            "Paints",
            tags,
            required,
            &format!("@paint_Name{class}"),
        )],
    )
}

fn port(port_tags: &str, required: &str, kind: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SItemPortDef")
            .value("Name", TestValue::text("hardpoint_paint"))
            .value("PortTags", TestValue::text(port_tags))
            .value("RequiredPortTags", TestValue::text(required))
            .value(
                "Types",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("SItemPortDefTypes").value("Type", TestValue::text(kind)),
                )]),
            ),
    )
}

fn ship(
    guid: u128,
    class: &str,
    definition: &str,
    modification: &str,
    ports: Vec<TestValue>,
) -> TestRecord {
    tagged_ship(guid, class, definition, modification, ports, "")
}

fn tagged_ship(
    guid: u128,
    class: &str,
    definition: &str,
    modification: &str,
    ports: Vec<TestValue>,
    tags: &str,
) -> TestRecord {
    let mut components = vec![TestValue::Struct(
        TestInstance::new("VehicleComponentParams")
            .value("vehicleDefinition", TestValue::text(definition))
            .value("modification", TestValue::text(modification))
            .value("vehicleName", TestValue::text("@vehicle_NameShip")),
    )];
    if !tags.is_empty() {
        components.push(attach("PowerPlant", tags, "", "@vehicle_NameShip"));
    }
    if !ports.is_empty() {
        components.push(TestValue::Struct(
            TestInstance::new("SItemPortContainerComponentParams")
                .value("Ports", TestValue::Array(ports)),
        ));
    }
    entity(guid, class, &format!("spaceships/{class}"), components)
}

fn database() -> DataForgeBuilder {
    let base = builder()
        .record(
            TestRecord::new(DATABASE, "TagDatabase", "TagDatabase.TagDatabase", TAG_PATH)
                .value("tags", references(&[TAG_FAMILY])),
        )
        .record(tag(TAG_FAMILY, "Paint_Alpha", &[TAG_VARIANT]))
        .record(tag(TAG_VARIANT, "Paint_Alpha_Variant", &[]))
        .record(entity(
            PLANT,
            "POWR_Test_Plant",
            "scitem/ships/powerplant/powr_plant",
            vec![attach("PowerPlant", "Paint_Alpha", "", "@plant_Name")],
        ));
    paints()
        .into_iter()
        .chain(ships())
        .fold(base, DataForgeBuilder::record)
}

fn paints() -> Vec<TestRecord> {
    [
        (
            PAINT_ALPHA_RED,
            "paint_alpha_red",
            "Paint_Alpha @Paint_Alpha_Red",
        ),
        (
            PAINT_ALPHA_VARIANT,
            "paint_alpha_variant",
            "Paint_Alpha_Variant",
        ),
        (PAINT_BETA, "paint_beta", "Paint_Beta"),
        (PAINT_GAMMA, "paint_gamma", "Paint_Gamma"),
        (PAINT_ORPHAN, "paint_orphan", "Paint_Orphan"),
        (PAINT_UPPER, "paint_upper", "PAINT_ALPHA"),
        (PAINT_TEST, "paint_test", "Paint_Alpha"),
    ]
    .into_iter()
    .map(|(guid, class, tags)| paint(guid, class, tags, ""))
    .chain([
        paint(PAINT_ZETA, "paint_zeta", "Paint_Zeta", "Paint_Zeta_Port"),
        paint(PAINT_ZETA_REQ, "paint_zeta_req", "Paint_Zeta", "Paint_Zeta"),
        paint(
            PAINT_ZETA_CASE,
            "paint_zeta_case",
            "Paint_Zeta",
            "PAINT_ZETA_PORT",
        ),
        paint(
            PAINT_ZETA_TWO,
            "paint_zeta_two",
            "Paint_Zeta",
            "Paint_Zeta_Port Mil",
        ),
        paint(PAINT_ETA_TREE, "paint_eta_tree", "Paint_Eta", "Paint_Alpha"),
        paint(PAINT_DELTA_MIL, "paint_delta_mil", "Paint_Delta", "Mil"),
        paint(
            PAINT_DELTA_PORT,
            "paint_delta_port",
            "Paint_Delta",
            "Paint_Delta",
        ),
    ])
    .collect()
}

fn ships() -> Vec<TestRecord> {
    vec![
        ship(SHIP_ALPHA, "ACAS_Alpha", ALPHA_XML, "", Vec::new()),
        ship(SHIP_BETA, "ACAS_Alpha_Beta", ALPHA_XML, "Beta", Vec::new()),
        ship(
            SHIP_GAMMA,
            "ACAS_Gamma",
            "",
            "",
            vec![
                port("Paint_Gamma", "Paint_Gamma", "Paints"),
                port("Paint_Orphan", "Paint_Orphan", "WeaponGun"),
            ],
        ),
        tagged_ship(
            SHIP_DELTA,
            "ACAS_Delta",
            "",
            "",
            vec![port("Paint_Delta", "Paint_Delta", "Paints")],
            "Mil",
        ),
        tagged_ship(
            SHIP_EPSILON,
            "ACAS_Epsilon",
            "",
            "",
            vec![port("Paint_Delta", "Paint_Delta", "Paints")],
            "Civ",
        ),
        tagged_ship(
            SHIP_ZETA,
            "ACAS_Zeta",
            "",
            "",
            vec![port("Paint_Zeta_Port", "Paint_Zeta", "Paints")],
            "",
        ),
        tagged_ship(
            SHIP_ETA,
            "ACAS_Eta",
            "",
            "",
            vec![port("Paint_Eta", "Paint_Eta", "Paints")],
            "Paint_Alpha_Variant",
        ),
        ship(
            SHIP_BROKEN,
            "ACAS_Broken",
            "scripts/entities/vehicles/implementations/xml/acas_broken.xml",
            "",
            Vec::new(),
        ),
        ship(
            SHIP_MISSING,
            "ACAS_Missing",
            "scripts/entities/vehicles/implementations/xml/acas_missing.xml",
            "",
            Vec::new(),
        ),
    ]
}

fn port_element(kind: &str, tags: &str) -> TestElement {
    TestElement::new("Part")
        .attribute("class", "ItemPort")
        .child(
            TestElement::new("ItemPort")
                .attribute("id", "modPaint")
                .attribute("portTags", tags)
                .attribute("requiredTags", tags)
                .child(
                    TestElement::new("Types")
                        .child(TestElement::new("Type").attribute("type", kind)),
                ),
        )
}

fn vehicle_xml() -> Vec<u8> {
    TestElement::new("Vehicle")
        .child(
            TestElement::new("Parts")
                .child(port_element("Paints", "Paint_Alpha"))
                .child(port_element("WeaponGun", "Paint_Orphan")),
        )
        .child(
            TestElement::new("Modifications").child(
                TestElement::new("Modification")
                    .attribute("name", "Beta")
                    .child(
                        TestElement::new("Elems").child(
                            TestElement::new("Elem")
                                .attribute("idRef", "modPaint")
                                .attribute("name", "requiredTags")
                                .attribute("value", "Paint_Beta"),
                        ),
                    ),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("paint_Namepaint_alpha_red", "Alpha Red")
            .text("vehicle_NameShip", "Ship")
            .build(),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(english())
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACAS_Alpha.xml"),
            vehicle_xml(),
        ))
        .entry(TestEntry::zstd(
            format!("{XML_DIR}ACAS_Broken.xml"),
            b"CryXmlB\0not really".to_vec(),
        ))
        .write_to(&path)?;
    Ok(path)
}
