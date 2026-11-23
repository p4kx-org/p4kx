// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with starmap objects and the system containers that place them.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, GlobalIniBuilder,
    TestContainer, TestElement, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

pub(crate) const STAR: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PLANET: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const MOON: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const STATION: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const JUMP: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const BELT: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const ORPHAN: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const BAD: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const MARKER_TEST: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const DOUBLE: u128 = 0x1000_0000_0000_4000_8000_0000_0000_000a;
pub(crate) const PLANET_TYPE: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const STATION_TYPE: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0002;

const RECORDS: &str = "libs/foundry/records/starmap/pu/system/stanton/";
const TYPES: &str = "libs/foundry/records/starmap/types/";
const CONTAINERS: &str = "Data\\ObjectContainers\\PU\\system\\";
const HALF_ROOT: &str = "0.70710678118654752440";

pub(crate) fn guid_text(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

pub(crate) fn database() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObjectType").field("name", TestType::String))
        .structure(
            TestStruct::new("StarMapObject")
                .field("name", TestType::Locale)
                .field("type", TestType::ReferenceTo("StarMapObjectType".into()))
                .field("parent", TestType::ReferenceTo("StarMapObject".into()))
                .field("hideInStarmap", TestType::Bool),
        )
        .record(kind(PLANET_TYPE, "Planet"))
        .record(kind(STATION_TYPE, "Station"));
    [
        object(STAR, "Stanton", None, None),
        object(PLANET, "Hurston", Some(PLANET_TYPE), Some(STAR)),
        object(MOON, "Aberdeen", Some(PLANET_TYPE), Some(PLANET)),
        object(STATION, "Everus", Some(STATION_TYPE), Some(PLANET)),
        object(JUMP, "Pyro", None, None),
        object(BELT, "Belt", None, Some(STAR)),
        object(ORPHAN, "Orphan", None, None),
        object(BAD, "Bad", None, None),
        object(MARKER_TEST, "Marker_Test", None, None),
        object(DOUBLE, "Double", None, None),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::record)
}

fn kind(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "StarMapObjectType",
        format!("StarMapObjectType.{name}"),
        format!("{TYPES}{}.xml", name.to_lowercase()),
    )
    .value("name", TestValue::text(name))
}

fn object(guid: u128, name: &str, kind: Option<u128>, parent: Option<u128>) -> TestRecord {
    let mut record = TestRecord::new(
        guid,
        "StarMapObject",
        format!("StarMapObject.{name}"),
        format!("{RECORDS}{}.xml", name.to_lowercase()),
    )
    .value("name", TestValue::text(format!("@starmap_{name}")))
    .value("hideInStarmap", TestValue::Bool(name == "Belt"));
    if let Some(kind) = kind {
        record = record.value("type", TestValue::Reference(kind));
    }
    if let Some(parent) = parent {
        record = record.value("parent", TestValue::Reference(parent));
    }
    record
}

pub(crate) fn child(position: &str, record: Option<u128>, entity_name: &str) -> TestElement {
    let mut child = TestElement::new("Child")
        .attribute("pos", position)
        .attribute("entityName", entity_name);
    if let Some(record) = record {
        child = child.attribute("starMapRecord", guid_text(record));
    }
    child
}

pub(crate) fn children(list: Vec<TestElement>) -> TestElement {
    list.into_iter().fold(
        TestElement::new("ChildObjectContainers"),
        TestElement::child,
    )
}

fn turned(child: TestElement) -> TestElement {
    child.attribute("rot", format!("{HALF_ROOT},0,0,{HALF_ROOT}"))
}

fn stanton_system() -> Vec<u8> {
    let planet = turned(child("1000,0,0", Some(PLANET), "planet_hurston"))
        .attribute("external", "1")
        .attribute(
            "name",
            "Data/objectcontainers/pu/system/stanton/stanton1.socpak",
        )
        .child(children(vec![child("0,2,0", Some(MOON), "moon_aberdeen")]));
    let group = turned(child("100,0,0", None, "SD_group")).child(children(vec![child(
        "1,0,0",
        Some(BELT),
        "belt",
    )]));
    TestElement::new("ObjectContainer")
        .child(children(vec![
            planet,
            child("0,0,50", Some(JUMP), "jumppoint_stanton_pyro"),
            child("7,0,0", Some(DOUBLE), "double_a"),
            group,
        ]))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn stanton_planet() -> String {
    TestElement::new("ObjectContainer")
        .child(children(vec![child("10,0,0", Some(STATION), "station")]))
        .to_xml()
}

fn terra_system() -> String {
    let missing = child("0,0,0", None, "missing")
        .attribute("external", "1")
        .attribute(
            "name",
            "Data/objectcontainers/pu/system/terra/missing.socpak",
        );
    TestElement::new("ObjectContainer")
        .child(children(vec![
            child("0,0,50.5", Some(JUMP), "jumppoint_again"),
            child("9,0,0", Some(DOUBLE), "double_b"),
            child("a,b,c", Some(BAD), "bad"),
            missing,
        ]))
        .to_xml()
}

pub(crate) fn socpak(name: &str, xml: impl Into<Vec<u8>>) -> Vec<u8> {
    TestContainer::new()
        .entry(TestEntry::stored(name, xml))
        .build()
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    builder().write_to(&path)?;
    Ok(path)
}

/// Returns the builder of the fixture Archive, for a test to add Entries to.
pub(crate) fn builder() -> ArchiveBuilder {
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new()
                .text("starmap_Hurston", "Hurston")
                .text("starmap_Aberdeen", "Aberdeen")
                .build(),
        ))
        .entry(TestEntry::stored(
            format!("{CONTAINERS}stanton\\stantonsystem.socpak"),
            socpak("stantonsystem.xml", stanton_system()),
        ))
        .entry(TestEntry::stored(
            format!("{CONTAINERS}stanton\\stanton1.socpak"),
            socpak("stanton1.xml", stanton_planet()),
        ))
        .entry(TestEntry::stored(
            format!("{CONTAINERS}terra\\terrasystem.socpak"),
            socpak("terrasystem.xml", terra_system()),
        ))
        .entry(TestEntry::stored(
            format!("{CONTAINERS}broken\\brokensystem.socpak"),
            b"not a container".to_vec(),
        ))
}
