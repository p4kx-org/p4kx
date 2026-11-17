// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue,
};

use super::{PLANET, PRESET};

const PLANET_RECORD: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
const PRESET_RECORD: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0004;
const ELEMENT_RECORD: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0005;
const COMPOSITION_RECORD: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0006;
const REGION: &str = "Data/ObjectContainers/PU/system/t/";

pub(super) fn archive(path: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let tile = format!("{REGION}tile.socpak");
    let root_xml = format!(
        r#"<ObjectContainer><ChildObjectContainers>
        <Child starMapRecord="{PLANET}" pos="10,0,0"><ChildObjectContainers>
        <Child name="{tile}" external="1" pos="1,0,0"/>
        <Child name="{tile}" external="1" pos="2,0,0"/>
        </ChildObjectContainers></Child>
        </ChildObjectContainers></ObjectContainer>"#,
    );
    let root = TestContainer::new().entry(TestEntry::stored("tsystem.xml", root_xml.as_bytes()));
    let marker = format!("\0preset\0{PRESET}\0");
    let body = |name| {
        TestContainer::new()
            .entry(TestEntry::stored(
                format!("{name}.xml"),
                b"<ObjectContainer/>",
            ))
            .entry(TestEntry::stored(format!("{name}.soc"), marker.as_bytes()))
            .build()
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dataforge()))
        .entry(TestEntry::stored(
            "Data/Localization/english/global.ini",
            b"planetoid_name=Synthetic planet\ndeposit_name=Synthetic deposit\n",
        ))
        .entry(TestEntry::stored(
            "Data/Localization/german/global.ini",
            b"planetoid_name=Testplanet\n",
        ))
        .entry(TestEntry::stored(
            format!("{REGION}tsystem.socpak"),
            root.build(),
        ))
        .entry(TestEntry::stored(tile, body("tile")))
        .entry(TestEntry::stored(
            format!("{REGION}unreached.socpak"),
            body("unreached"),
        ))
        .write_to(path)?;
    Ok(Archive::open(path)?)
}

fn dataforge() -> Vec<u8> {
    let element = TestStruct::new("MineableElement").field("elementInstability", TestType::Single);
    let part = TestStruct::new("CompositionPart")
        .field("mineableElement", TestType::Reference)
        .field("minPercentage", TestType::Single);
    let composition = TestStruct::new("MineableComposition")
        .field("depositName", TestType::Locale)
        .array(
            "compositionArray",
            TestType::Class("CompositionPart".to_owned()),
        );
    let provider = TestStruct::new("HarvestableProviderPreset").field("unused", TestType::Bool);
    let starmap = TestStruct::new("StarMapObject").field("name", TestType::String);
    DataForgeBuilder::new()
        .version(8)
        .structure(element)
        .structure(part)
        .structure(composition)
        .structure(provider)
        .structure(starmap)
        .record(
            TestRecord::new(
                ELEMENT_RECORD,
                "MineableElement",
                "MineableElement.Synthetic",
                "libs/foundry/records/mining/mineableelements/synthetic.xml",
            )
            .value("elementInstability", TestValue::Single(0.75)),
        )
        .record(
            TestRecord::new(
                COMPOSITION_RECORD,
                "MineableComposition",
                "MineableComposition.Synthetic",
                "libs/foundry/records/mining/rockcompositionpresets/synthetic.xml",
            )
            .value("depositName", TestValue::text("@deposit_name"))
            .value(
                "compositionArray",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("CompositionPart")
                        .value("mineableElement", TestValue::Reference(ELEMENT_RECORD))
                        .value("minPercentage", TestValue::Single(20.0)),
                )]),
            ),
        )
        .record(TestRecord::new(
            PRESET_RECORD,
            "HarvestableProviderPreset",
            "HarvestableProviderPreset.Synthetic",
            "libs/foundry/records/harvestable/providerpresets/synthetic.xml",
        ))
        .record(
            TestRecord::new(
                PLANET_RECORD,
                "StarMapObject",
                "StarMapObject.Synthetic",
                "libs/foundry/records/starmap/pu/synthetic.xml",
            )
            .value("name", TestValue::text("@planetoid_name")),
        )
        .build()
}
