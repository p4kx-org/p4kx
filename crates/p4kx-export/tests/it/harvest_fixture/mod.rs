// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with mining and harvestable Records and the containers that name a
//! provider preset in their `.soc`.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestContainer, TestElement, TestEntry,
    TestInstance, TestRecord, TestValue,
};

use crate::starmap_fixture::{child, children, guid_text};

mod mineables;
mod structures;

pub(crate) use mineables::Radar;
use structures::structures;

pub(crate) const RESOURCE: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const ELEMENT: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const COMPOSITION: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0003;
const COMPOSITION_TEST: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const PARAMS: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0005;
const PARAMS_TEST: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0006;
pub(crate) const ENTITY: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0007;
pub(crate) const HARVESTABLE: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0008;
pub(crate) const PRESET_PLACED: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0009;
pub(crate) const PRESET_PRIVATE: u128 = 0x7000_0000_0000_4000_8000_0000_0000_000a;
pub(crate) const PRESET_UNREACHED: u128 = 0x7000_0000_0000_4000_8000_0000_0000_000b;
pub(crate) const PRESET_NOWHERE: u128 = 0x7000_0000_0000_4000_8000_0000_0000_000c;
pub(crate) const MULTI_CONFIG: u128 = 0x7000_0000_0000_4000_8000_0000_0000_000d;
pub(crate) const ENTITY_SILENT: u128 = 0x7000_0000_0000_4000_8000_0000_0000_000e;
pub(crate) const ENTITY_BARE: u128 = 0x7000_0000_0000_4000_8000_0000_0000_000f;
pub(crate) const RADAR_GLOBAL: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0010;
pub(crate) const CONTACT_TYPE: u128 = 0x7000_0000_0000_4000_8000_0000_0000_0011;
pub(crate) const PLANET: u128 = 0x7100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PRIVATE_POINT: u128 = 0x7100_0000_0000_4000_8000_0000_0000_0002;

const MINING: &str = "libs/foundry/records/mining/";
const PRESETS: &str = "libs/foundry/records/harvestable/providerpresets/system/stanton/";
const SLOTS: &str = "libs/foundry/records/harvestable/slotpresets/";
const CONTAINERS: &str = "Data\\ObjectContainers\\PU\\system\\stanton\\";

fn record(guid: u128, type_name: &str, name: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        type_name,
        format!("{type_name}.{name}"),
        format!("{path}.xml"),
    )
}

fn part(minimum: f32, element: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new("MineableCompositionPart")
            .value("minPercentage", TestValue::Single(minimum))
            .value("maxPercentage", TestValue::Single(70.0))
            .value("probability", TestValue::Single(0.8))
            .value("mineableElement", TestValue::Reference(element)),
    )
}

fn composition(guid: u128, name: &str) -> TestRecord {
    record(
        guid,
        "MineableComposition",
        name,
        &format!("{MINING}rockcompositionpresets/{}", name.to_lowercase()),
    )
    .value(
        "depositName",
        TestValue::text("@hud_mining_asteroid_name_5"),
    )
    .value("minimumDistinctElements", TestValue::Int32(2))
    .value(
        "compositionArray",
        TestValue::Array(vec![part(30.0, ELEMENT), part(20.0, RESOURCE + 0xff)]),
    )
}

fn params(guid: u128, name: &str, mass: f32) -> TestRecord {
    record(
        guid,
        "MiningGlobalParams",
        name,
        &format!("{MINING}{}", name.to_lowercase()),
    )
    .value("cSCUPerVolume", TestValue::Single(0.7))
    .value("compositionMassMultiplier", TestValue::Single(1.5))
    .value("defaultMass", TestValue::Single(mass))
    .value(
        "hitConsistencyParams",
        TestValue::Struct(
            TestInstance::new("HitConsistencyParams").value("timeExponent", TestValue::Single(2.0)),
        ),
    )
    .value("wasteResourceType", TestValue::Reference(RESOURCE))
}

fn harvestable_entry(harvestable: Option<u128>, entity: Option<u128>) -> TestValue {
    let mut element = TestInstance::new("HarvestableElement")
        .value("relativeProbability", TestValue::Single(22.0))
        .value("clustering", TestValue::Reference(RESOURCE))
        .value(
            "geometries",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("HarvestableGeometry")
                    .value("tag", TestValue::Reference(RESOURCE)),
            )]),
        );
    if let Some(harvestable) = harvestable {
        element = element.value("harvestable", TestValue::Reference(harvestable));
    }
    if let Some(entity) = entity {
        element = element.value("harvestableEntityClass", TestValue::Reference(entity));
    }
    TestValue::Struct(element)
}

fn provider(guid: u128, name: &str, with_entity_only: bool) -> TestRecord {
    let mut elements = vec![harvestable_entry(Some(HARVESTABLE), None)];
    if with_entity_only {
        elements.push(harvestable_entry(None, Some(ENTITY)));
    }
    record(
        guid,
        "HarvestableProviderPreset",
        name,
        &format!("{PRESETS}{}", name.to_lowercase()),
    )
    .value(
        "harvestableGroups",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("HarvestableElementGroup")
                .value("groupName", TestValue::text("SpaceShip_Mineables"))
                .value("groupProbability", TestValue::Single(6.0))
                .value("harvestables", TestValue::Array(elements)),
        )]),
    )
    .value(
        "areas",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("HarvestableAreaPreset")
                .value("debugGroupName", TestValue::text("Acidic"))
                .value("globalModifier", TestValue::Single(1.0))
                .value(
                    "areaType",
                    TestValue::Struct(TestInstance::new("HarvestableAreaTypeObjectPreset").value(
                        "objectPresetPaths",
                        TestValue::Array(vec![TestValue::text(
                            "Libs/ObjectPresets/PU/system/stanton/hurston/acidic_pools_01.opr",
                        )]),
                    )),
                ),
        )]),
    )
}

#[expect(clippy::too_many_lines, reason = "one value per field of the fixture")]
fn multi_config() -> TestRecord {
    let sub = TestValue::Struct(
        TestInstance::new("SubHarvestableEntry")
            .value("harvestable", TestValue::Reference(HARVESTABLE))
            .value("harvestableRespawnTimeMultiplier", TestValue::Single(1.0))
            .value("relativeProbability", TestValue::Single(2.0))
            .value(
                "relativeProbabilityDeepest",
                TestValue::Struct(
                    TestInstance::new("OptionalProbability")
                        .value("probability", TestValue::Single(0.15)),
                ),
            ),
    );
    let tagged = TestValue::Struct(
        TestInstance::new("TaggedSubConfig")
            .value("name", TestValue::text("Rock"))
            .value(
                "tagList",
                TestValue::Struct(TestInstance::new("HarvestableTagList").value(
                    "tags",
                    TestValue::Array(vec![TestValue::Reference(RESOURCE)]),
                )),
            )
            .value(
                "subConfig",
                TestValue::Struct(
                    TestInstance::new("SubHarvestableConfigSingleManual").value(
                        "subConfigManual",
                        TestValue::Struct(
                            TestInstance::new("SubHarvestableManual")
                                .value("configRespawnTimeMultiplier", TestValue::Single(1.0))
                                .value("initialSlotsProbability", TestValue::Single(0.5))
                                .value("subHarvestables", TestValue::Array(vec![sub])),
                        ),
                    ),
                ),
            ),
    );
    record(
        MULTI_CONFIG,
        "SubHarvestableMultiConfigRecord",
        "SlotPreset_Cave",
        &format!("{SLOTS}caves/slotpreset_cave"),
    )
    .value(
        "multiConfig",
        TestValue::Struct(
            TestInstance::new("SubHarvestableMultiConfig")
                .value(
                    "defaultConfig",
                    TestValue::Struct(
                        TestInstance::new("SubHarvestableConfigSingleRef")
                            .value("subConfigRef", TestValue::Reference(MULTI_CONFIG)),
                    ),
                )
                .value(
                    "ignoreAttachableTagsForTaggedConfigs",
                    TestValue::Bool(true),
                )
                .value("taggedConfigs", TestValue::Array(vec![tagged])),
        ),
    )
}

fn database(radar: Radar) -> DataForgeBuilder {
    mineables::add(structures(DataForgeBuilder::new().version(8)), radar)
        .record(record(
            RESOURCE,
            "ResourceType",
            "iron",
            "libs/foundry/records/resourcetypedatabase/iron",
        ))
        .record(
            record(
                ELEMENT,
                "MineableElement",
                "iron_ore",
                &format!("{MINING}mineableelements/iron_ore"),
            )
            .value("resourceType", TestValue::Reference(RESOURCE)),
        )
        .record(composition(COMPOSITION, "Asteroid_CType_Iron"))
        .record(composition(COMPOSITION_TEST, "Composition_Test"))
        .record(params(PARAMS, "MiningGlobalParamsShip", 100.0))
        .record(params(PARAMS_TEST, "MiningGlobalParams_Test", 1.0))
        .record(
            record(
                HARVESTABLE,
                "HarvestablePreset",
                "Mining_Iron",
                "libs/foundry/records/harvestable/harvestablepresets/mining_iron",
            )
            .value("entityClass", TestValue::Reference(ENTITY)),
        )
        .record(provider(PRESET_PLACED, "HPP_Placed", true))
        .record(provider(PRESET_PRIVATE, "HPP_Private", false))
        .record(provider(PRESET_UNREACHED, "HPP_Unreached", false))
        .record(provider(PRESET_NOWHERE, "HPP_Nowhere", false))
        .record(multi_config())
        .record(
            record(
                PLANET,
                "StarMapObject",
                "Stanton1",
                "libs/foundry/records/starmap/pu/system/stanton/stanton1",
            )
            .value("name", TestValue::text("@starmap_Stanton1")),
        )
        .record(
            record(
                PRIVATE_POINT,
                "StarMapObject",
                "Claim_01",
                "libs/foundry/records/starmap/pu/system/stanton/claim_01",
            )
            .value("name", TestValue::text("@starmap_Claim"))
            .value("description", TestValue::text("@MiningClaims_Desc_001")),
        )
}

fn external(name: &str, record: u128, inner: Vec<TestElement>) -> TestElement {
    let mut element = child("0,0,0", Some(record), name)
        .attribute("external", "1")
        .attribute(
            "name",
            format!("Data/objectcontainers/pu/system/stanton/{name}.socpak"),
        );
    if !inner.is_empty() {
        element = element.child(children(inner));
    }
    element
}

fn marked(preset: u128) -> Vec<u8> {
    [
        b"\0Entities\0preset\0".as_slice(),
        guid_text(preset).as_bytes(),
        b"\0",
    ]
    .concat()
}

fn container(name: &str, xml: String, soc: Vec<u8>) -> TestEntry {
    TestEntry::stored(
        format!("{CONTAINERS}{name}.socpak"),
        TestContainer::new()
            .entry(TestEntry::stored(format!("{name}.xml"), xml))
            .entry(TestEntry::stored(format!("{name}.soc"), soc))
            .build(),
    )
}

fn xml(list: Vec<TestElement>) -> String {
    TestElement::new("ObjectContainer")
        .child(children(list))
        .to_xml()
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_with(dir, Radar::Named)
}

/// Writes the fixture Archive whose radar parameters are `radar` into `dir`.
pub(crate) fn write_archive_with(dir: &Path, radar: Radar) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database(radar).build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new()
                .text("hud_mining_asteroid_name_5", "Iron Deposit")
                .build(),
        ))
        .entry(container(
            "stantonsystem",
            xml(vec![external("stanton1", PLANET, Vec::new())]),
            Vec::new(),
        ))
        .entry(container(
            "stanton1",
            xml(vec![external("pmp", PRIVATE_POINT, Vec::new())]),
            marked(PRESET_PLACED),
        ))
        .entry(container("pmp", xml(Vec::new()), marked(PRESET_PRIVATE)))
        .entry(container(
            "unreached",
            xml(Vec::new()),
            marked(PRESET_UNREACHED),
        ))
        .entry(container("plain", xml(Vec::new()), b"\0nothing\0".to_vec()))
        .write_to(&path)?;
    Ok(path)
}
