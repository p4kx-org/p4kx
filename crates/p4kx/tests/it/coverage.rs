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

fn database() -> DataForgeBuilder {
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
}

fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let archive = dir.join("Data.p4k");
    let vehicle = TestElement::new("Vehicle")
        .attribute("name", "Good")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(TestEntry::zstd(format!("{FOLDER}\\Good.xml"), vehicle))
        .write_to(&archive)?;
    Ok(archive)
}

fn coverage(args: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    coverage_at(&archive, args, code)
}

fn coverage_at(archive: &Path, args: &[&str], code: i32) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .arg("coverage")
        .arg(archive)
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

#[test]
fn json_binds_duplicate_paths_to_their_physical_sources() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let duplicate = format!("{FOLDER}\\duplicate.xml");
    let later = format!("{FOLDER}\\z.xml");
    write_origin_fixture(&archive, &duplicate, &later)?;
    let report: Value = serde_json::from_str(&coverage_at(&archive, &["--json"], 1)?)?;

    for (section, name, record) in [("types", "Faction", 1), ("starmap_types", "SolarSystem", 3)] {
        assert_sample_origin(&report, section, name, record)?;
    }
    for (row, (expected, physical)) in [(&duplicate, 2), (&duplicate, 3), (&later, 1)]
        .into_iter()
        .enumerate()
    {
        assert_failure_origin(&report, row, expected, physical)?;
    }
    Ok(())
}

// Two samples share filename bytes, and two failures share both Entry path and reason.
fn write_origin_fixture(archive: &Path, duplicate: &str, later: &str) -> TestResult {
    const SAMPLE_FILE: &str = "libs/factions/good.xml";
    let forge = database()
        .structure(TestStruct::new("StarMapObjectType").field("name", TestType::String))
        .structure(TestStruct::new("StarMapObject").field(
            "type",
            TestType::ReferenceTo("StarMapObjectType".to_owned()),
        ))
        .record(
            TestRecord::new(
                3,
                "StarMapObjectType",
                "StarMapObjectType.SolarSystem",
                "type.xml",
            )
            .value("name", TestValue::text("SolarSystem")),
        )
        .record(
            TestRecord::new(4, "StarMapObject", "StarMapObject.Sample", SAMPLE_FILE)
                .value("type", TestValue::Reference(3)),
        )
        .build();
    let invalid = b"not XML".to_vec();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge).encrypted())
        .entry(TestEntry::zstd(later, invalid.clone()))
        .entry(TestEntry::zstd(duplicate, invalid.clone()).encrypted())
        .entry(TestEntry::zstd(duplicate, invalid))
        .write_to(archive)?;
    Ok(())
}

fn assert_sample_origin(report: &Value, section: &str, name: &str, record: u64) -> TestResult {
    let row = report
        .get(section)
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter()
                .position(|row| text_at(row, "/name") == Some(name))
        })
        .ok_or("expected coverage sample row")?;
    let path = format!("/{section}/{row}/sample/record_path");
    assert_eq!(
        text_at(report, &path),
        Some("libs/factions/good"),
        "sample value for {path}"
    );
    let evidence = contributors(report, &path, true)?;
    assert_eq!(
        evidence.len(),
        2,
        "fileName and original text span for {path}"
    );
    for contributor in &evidence {
        assert_eq!(
            entry_index(report, contributor)?,
            0,
            "encrypted DCB physical Entry"
        );
        assert_eq!(
            text_at(contributor, "/role"),
            Some("conversion"),
            "converted filename for {path}"
        );
        assert_eq!(
            text_at(contributor, "/scope"),
            Some("payload"),
            "DCB payload scope for {path}"
        );
    }
    assert!(
        evidence.iter().any(|contributor| {
            contributor
                .pointer("/location/recordIndex")
                .and_then(Value::as_u64)
                == Some(record)
                && text_at(contributor, "/location/field") == Some("fileName")
        }),
        "exact Record header for {path}"
    );
    assert!(
        evidence.iter().any(|contributor| {
            text_at(contributor, "/location/namespace") == Some("dataforgeDecodedBytes")
                && text_at(contributor, "/location/field") == Some("text")
        }),
        "original filename bytes for {path}"
    );
    Ok(())
}

fn assert_failure_origin(report: &Value, row: usize, expected: &str, physical: u64) -> TestResult {
    let path = format!("/entries/0/failures/{row}/path");
    assert_eq!(
        text_at(report, &path),
        Some(expected),
        "ordered path {path}"
    );
    let evidence = contributors(report, &path, false)?;
    let [contributor] = evidence.as_slice() else {
        return Err("each failed Entry path must have its own directory contributor".into());
    };
    assert_eq!(
        entry_index(report, contributor)?,
        physical,
        "exact Entry ordered with {path}"
    );
    assert_eq!(
        text_at(contributor, "/scope"),
        Some("archiveDirectoryMetadata"),
        "directory path scope for {path}"
    );
    assert_eq!(
        text_at(contributor, "/role"),
        Some("payload"),
        "direct value for {path}"
    );
    assert_eq!(
        text_at(contributor, "/location/field"),
        Some("path"),
        "directory field for {path}"
    );
    let reason = format!("/entries/0/failures/{row}/reason");
    assert_eq!(
        text_at(report, &reason),
        Some("neither CryXml nor text XML"),
        "unchanged reason for {path}"
    );
    assert!(
        report
            .get("fieldOrigins")
            .and_then(|origins| origins.get(reason))
            .is_none(),
        "path proof does not attest the diagnostic"
    );
    Ok(())
}

fn contributors<'report>(
    report: &'report Value,
    path: &str,
    encrypted: bool,
) -> Result<Vec<&'report Value>, Box<dyn Error>> {
    let origin = report
        .get("fieldOrigins")
        .and_then(|origins| origins.get(path))
        .ok_or("missing exact field origin")?;
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(encrypted),
        "{path}"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "{path}"
    );
    let evidence = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source references")?
        .iter()
        .map(|reference| {
            evidence
                .iter()
                .find(|contributor| contributor.get("id") == Some(reference))
                .ok_or_else(|| format!("unresolved contributor for {path}").into())
        })
        .collect()
}

fn entry_index(report: &Value, contributor: &Value) -> Result<u64, Box<dyn Error>> {
    let source_id = contributor.get("source").ok_or("missing source ID")?;
    report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical inventory")?
        .iter()
        .find(|source| source.get("id") == Some(source_id))
        .and_then(|source| source.get("entryIndex").and_then(Value::as_u64))
        .ok_or_else(|| "contributor has no physical Entry index".into())
}

fn text_at<'value>(value: &'value Value, pointer: &str) -> Option<&'value str> {
    value.pointer(pointer).and_then(Value::as_str)
}
