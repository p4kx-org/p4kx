// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx coverage` over a synthetic Archive: the table, the JSON and the exit code.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntry,
    TestInstance, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const FOLDER: &str = "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml";

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Params"))
        .structure(TestStruct::new("SCItemWeaponComponentParams").parent("Params"))
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", TestType::StrongPointer("Params".to_owned())),
        )
        .structure(TestStruct::new("Faction").field("name", TestType::String))
        .record(
            TestRecord::new(
                1,
                "EntityClassDefinition",
                "Entity.Gun",
                "libs/entities/gun.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(TestInstance::new(
                    "SCItemWeaponComponentParams",
                ))]),
            ),
        )
        .record(
            TestRecord::new(2, "Faction", "Faction.Good", "libs/factions/good.xml")
                .value("name", TestValue::text("good")),
        )
        .build()
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let archive = dir.join("Data.p4k");
    let vehicle = TestElement::new("Vehicle")
        .attribute("name", "Good")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database()).encrypted())
        .entry(TestEntry::zstd(format!("{FOLDER}\\Good.xml"), vehicle))
        .write_to(&archive)?;
    Ok(archive)
}

fn coverage(args: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let output = p4kx_testkit::p4kx()?
        .arg("coverage")
        .arg(&archive)
        .args(args)
        .output()?;
    assert_eq!(output.status.code(), Some(code), "exit code: {output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

fn find<'a>(list: &'a Value, name: &str) -> Result<&'a Value, Box<dyn Error>> {
    list.as_array()
        .and_then(|items| items.iter().find(|item| item["name"] == name))
        .ok_or_else(|| format!("no {name} in {list}").into())
}

#[test]
fn json_lists_every_type_component_and_folder_with_counts() -> TestResult {
    let document: Value = serde_json::from_str(&coverage(&["--json"], 1)?)?;

    assert_eq!(document["complete"], false);
    assert_eq!(document["types"].as_array().map(Vec::len), Some(38));
    assert_eq!(document["components"].as_array().map(Vec::len), Some(20));
    let system = find(&document["starmap_types"], "SolarSystem")?;
    assert_eq!(system["area"], "starmap");
    assert_eq!(system["records"], 0);
    assert_eq!(system["sample"], Value::Null);
    let faction = find(&document["types"], "Faction")?;
    assert_eq!(faction["records"], 1);
    assert_eq!(faction["defined"], true);
    assert_eq!(faction["sample"]["record_path"], "libs/factions/good");
    let missing = find(&document["types"], "MissionItem")?;
    assert_eq!(missing["defined"], false);
    assert_eq!(missing["records"], 0);
    assert_eq!(missing["sample"], Value::Null);
    let weapon = find(&document["components"], "SCItemWeaponComponentParams")?;
    assert_eq!(weapon["entities"], 1);
    assert_eq!(weapon["instances"], 1);
    assert_eq!(weapon["reachable"], true);
    let [folder] = document["entries"].as_array().map_or_default(Vec::as_slice) else {
        return Err("one folder".into());
    };
    assert_eq!(folder["entries"], 1);
    assert_eq!(folder["converted"], 1);
    Ok(())
}

#[test]
fn the_table_has_a_section_per_kind() -> TestResult {
    let table = coverage(&[], 1)?;

    for heading in [
        "Record types",
        "Components of EntityClassDefinition",
        "StarMapObject Records by StarMapObjectType",
        "Entry folders",
    ] {
        assert!(table.contains(heading), "{heading}: {table}");
    }
    assert!(table.contains("Faction"), "{table}");
    assert!(table.contains("not defined"), "{table}");
    Ok(())
}

#[test]
fn json_and_a_table_format_exclude_each_other() -> TestResult {
    coverage(&["--json", "--format", "table"], 2)?;
    Ok(())
}
