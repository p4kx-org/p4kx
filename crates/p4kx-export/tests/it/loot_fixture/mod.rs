// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with loot tables, archetypes, pool filters, a multi config whose slot
//! generates loot, and containers that are placed with that config.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, GlobalIniBuilder,
    TestChunkFile, TestContainer, TestElement, TestEntry, TestInstance, TestRecord, TestValue,
};

use crate::health_origin_fixture::{Fixture, Input, REGISTRY, TestResult};
use crate::starmap_fixture::{child, children, guid_text};

mod legacy;
mod records;
mod structures;

use structures::structures;

pub(crate) const TAG_RARE: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TAG_COMMON: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const ENTITY: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const ARCHETYPE: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const TABLE: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0005;
pub(crate) const TABLE_DEBUG: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const LEGACY_TABLE: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const FILTER_LEAF: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const FILTER: u128 = 0x7200_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const CONFIG: u128 = 0x7200_0000_0000_4000_8000_0000_0000_000a;
pub(crate) const LEGACY_ARCHETYPE: u128 = 0x7200_0000_0000_4000_8000_0000_0000_000b;
pub(crate) const PLANET: u128 = 0x7300_0000_0000_4000_8000_0000_0000_0001;

pub(crate) const AREA: &str = "Data\\ObjectContainers\\PU\\loc\\loot\\area.socpak";
pub(crate) const NESTED: &str = "Data\\ObjectContainers\\PU\\loc\\loot\\nested.socpak";
const AREA_WRITTEN: &str = "ObjectContainers/PU/loc/loot/area.socpak";
pub(crate) const GHOST: &str = "ObjectContainers/PU/loc/loot/ghost.socpak";

const CONTAINERS: &str = "Data\\ObjectContainers\\PU\\system\\stanton\\";

pub(crate) fn database() -> DataForgeBuilder {
    records::records(structures(DataForgeBuilder::new().version(8)), &ids())
}

pub(crate) fn database_with_loot_aliases() -> DataForgeBuilder {
    database()
        .record(records::table_record(TABLE, "V3LootTable_Alias", &ids()))
        .record(TestRecord::new(
            TABLE,
            "LootTableV3Record",
            "LootTableV3Record.Outside",
            "unselected/loottables/outside.xml",
        ))
}

pub(crate) fn database_with_legacy_loot_alias() -> DataForgeBuilder {
    let broken_link = TestInstance::new("WeightedLootArchetype")
        .value("weight", TestValue::Single(0.5))
        .value("archetype", TestValue::Reference(u128::MAX));
    database()
        .record(
            TestRecord::new(
                LEGACY_TABLE,
                "LootTable",
                "LootTable.LegacyAlias",
                "libs/foundry/records/lootgeneration/loottables/legacy_alias.xml",
            )
            .value("lootArchetypes", array(vec![broken_link])),
        )
        .record(TestRecord::new(
            LEGACY_TABLE,
            "LootTable",
            "LootTable.Outside",
            "unselected/loottables/legacy_outside.xml",
        ))
}

pub(crate) fn source_fixture(data: &[u8], input: Input) -> TestResult<Fixture> {
    Fixture::open(
        data,
        input,
        vec![TestEntry::stored(REGISTRY, b"<LayerBackups/>")],
    )
}

const fn ids() -> records::Ids {
    records::Ids {
        tag_rare: TAG_RARE,
        tag_common: TAG_COMMON,
        entity: ENTITY,
        archetype: ARCHETYPE,
        table: TABLE,
        table_debug: TABLE_DEBUG,
        legacy_table: LEGACY_TABLE,
        legacy_archetype: LEGACY_ARCHETYPE,
        filter_leaf: FILTER_LEAF,
        filter: FILTER,
        config: CONFIG,
        planet: PLANET,
    }
}

fn xml(list: Vec<TestElement>) -> String {
    TestElement::new("ObjectContainer")
        .child(children(list))
        .to_xml()
}

/// The property file of an entity that places `container` and carries `config`.
fn placing(container: &str, config: u128) -> String {
    TestElement::new("Entity")
        .attribute("Name", "loot")
        .child(
            TestElement::new("PropertiesDataCore")
                .child(
                    TestElement::new("EntityComponentObjectContainer")
                        .attribute("objectContainer", container),
                )
                .child(
                    TestElement::new("HarvestableComponent").child(
                        TestElement::new("looseSubConfigBase")
                            .attribute("multiConfigRef", guid_text(config)),
                    ),
                ),
        )
        .to_xml()
}

/// The property file of an entity that names two different containers and no config.
fn naming_two(first: &str, second: &str) -> String {
    TestElement::new("Entity")
        .child(TestElement::new("A").attribute("objectContainer", first))
        .child(TestElement::new("B").attribute("objectContainer", second))
        .to_xml()
}

fn entity_files(files: &[String]) -> Vec<TestEntry> {
    files
        .iter()
        .enumerate()
        .map(|(at, text)| TestEntry::stored(format!("entdata/{at}.entxml"), text.clone()))
        .collect()
}

fn tagged(name: &str, tags: &[u128]) -> TestElement {
    let list = tags.iter().fold(TestElement::new("Tags"), |list, tag| {
        list.child(TestElement::new("Tag").attribute("TagId", guid_text(*tag)))
    });
    TestElement::new("Entity")
        .attribute("Name", name)
        .child(list)
}

/// A `.soc` file whose entities carry the listed tags, one list per entity.
fn soc(entities: &[&[u128]]) -> Vec<u8> {
    let root = entities
        .iter()
        .enumerate()
        .fold(TestElement::new("Entities"), |root, (at, tags)| {
            root.child(tagged(&format!("point{at}"), tags))
        });
    TestChunkFile::crch()
        .chunk(
            0x0004,
            3,
            root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
        )
        .bytes()
}

fn container(path: &str, xml: String, files: &[String], socs: Vec<Vec<u8>>) -> TestEntry {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let stem = name.trim_end_matches(".socpak");
    let mut inner = TestContainer::new().entry(TestEntry::stored(format!("{stem}.xml"), xml));
    for (at, soc) in socs.into_iter().enumerate() {
        inner = inner.entry(TestEntry::stored(format!("{stem}_{at}.soc"), soc));
    }
    for file in entity_files(files) {
        inner = inner.entry(file);
    }
    TestEntry::stored(path, inner.build())
}

fn system(name: &str, xml: String, files: &[String]) -> TestEntry {
    container(
        &format!("{CONTAINERS}{name}.socpak"),
        xml,
        files,
        Vec::new(),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    let stanton1 = child("0,0,0", Some(PLANET), "stanton1")
        .attribute("external", "1")
        .attribute(
            "name",
            "Data/objectcontainers/pu/system/stanton/stanton1.socpak",
        );
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new().build(),
        ))
        .entry(system("stantonsystem", xml(vec![stanton1]), &[]))
        .entry(system(
            "stanton1",
            xml(Vec::new()),
            &[
                placing("ObjectContainers/PU/loc/loot/area.socpak", CONFIG),
                placing(GHOST, CONFIG),
            ],
        ))
        .entry(system(
            "unreached",
            xml(Vec::new()),
            &[placing("objectcontainers/pu/loc/loot/area.socpak", CONFIG)],
        ))
        .entry(container(
            AREA,
            xml(Vec::new()),
            &[placing(
                "ObjectContainers/PU/loc/loot/nested.socpak",
                CONFIG,
            )],
            vec![
                soc(&[&[TAG_COMMON, TAG_RARE], &[TAG_COMMON], &[TAG_COMMON], &[]]),
                soc(&[&[TAG_RARE]]),
            ],
        ))
        .entry(container(
            NESTED,
            xml(Vec::new()),
            &[
                placing(AREA_WRITTEN, CONFIG),
                naming_two(AREA_WRITTEN, GHOST),
            ],
            Vec::new(),
        ))
        .write_to(&path)?;
    Ok(path)
}

/// Returns a `TestInstance` of `type_name`; shorthand for the record builders.
pub(super) fn instance(type_name: &str) -> TestInstance {
    TestInstance::new(type_name)
}

/// Returns an array of the structs in `list`.
pub(super) fn array(list: Vec<TestInstance>) -> TestValue {
    TestValue::Array(list.into_iter().map(TestValue::Struct).collect())
}

/// Returns a record of `type_name` named `name` at `path` (without `.xml`).
pub(super) fn record(guid: u128, type_name: &str, name: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        type_name,
        format!("{type_name}.{name}"),
        format!("{path}.xml"),
    )
}
