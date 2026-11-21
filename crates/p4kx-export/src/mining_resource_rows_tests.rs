// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;

use crate::mining_locations::{
    MiningLocationOptions, MiningLocationSnapshot, MiningResourceType, read_mining_locations,
};
use crate::{CatalogOptions, build_catalog};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const ORE: u128 = 0x3;
const REFINED: u128 = 0x4;
const UNREACHED: u128 = 0x5;
const ELEMENT: u128 = 0x6;

#[test]
fn resource_dependencies_match_canonical_rows_and_close_both_display_names() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = archive(&folder.path().join("Data.p4k"), true)?;
    let snapshot = read_mining_locations(
        &archive,
        MiningLocationOptions::new(Language::English, None),
    )?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_dependencies(&snapshot, &catalog)?;
    Ok(())
}

fn check_dependencies(snapshot: &MiningLocationSnapshot, catalog: &Value) -> TestResult {
    let [ore, refined] = snapshot.resource_types.as_slice() else {
        return Err("only the ore and one direct refined product are dependencies".into());
    };
    assert_eq!(
        ore.source.record_index.value(),
        Some(&0),
        "the ore uses its original Record"
    );
    assert_eq!(
        refined.source.record_index.value(),
        Some(&1),
        "the refined product uses its original Record"
    );
    let link = ore
        .refined_version
        .value()
        .ok_or("authored refined version")?;
    assert_eq!(
        link.target_record_index().value(),
        refined.source.record_index.value(),
        "the refinement points at the returned physical resource row"
    );
    assert_eq!(
        snapshot.coverage.unresolved_resource_links, 0,
        "both reachable dependency links resolve"
    );
    assert_eq!(
        snapshot.coverage.unreadable_resource_types, 0,
        "all requested resource rows are readable"
    );
    let definitions = catalog
        .pointer("/definitions/resourceTypes/rows")
        .and_then(Value::as_array)
        .ok_or("resource definitions")?;
    let names = catalog
        .pointer("/texts/publicNames")
        .and_then(Value::as_array)
        .ok_or("public names")?;
    assert_eq!(
        snapshot.public_names.len(),
        2,
        "only dependency names are returned"
    );
    for row in [ore, refined] {
        check_row(snapshot, definitions, names, row)?;
    }
    Ok(())
}

fn check_row(
    snapshot: &MiningLocationSnapshot,
    definitions: &[Value],
    names: &[Value],
    row: &MiningResourceType,
) -> TestResult {
    let actual = serde_json::to_value(row)?;
    let expected = definitions
        .iter()
        .find(|definition| definition["recordIndex"] == actual["recordIndex"])
        .ok_or("physical catalog definition")?;
    for (key, field) in actual.as_object().ok_or("typed resource row")? {
        assert_eq!(field, &expected[key], "canonical resource value {key}");
    }
    let id = row.name.value().ok_or("authored resource name")?.as_str();
    let text = snapshot
        .public_names
        .iter()
        .find(|text| text.id.value().is_some_and(|name| name.as_str() == id))
        .ok_or("query public name")?;
    let expected = names
        .iter()
        .find(|name| name.pointer("/id/value").and_then(Value::as_str) == Some(id))
        .ok_or("canonical public name")?;
    assert_eq!(
        &serde_json::to_value(text)?,
        expected,
        "the resolved name matches the Catalog"
    );
    for origin in [
        row.name.origin(),
        row.refined_version.origin(),
        text.key.origin(),
    ] {
        for use_id in origin.source_uses_refs() {
            assert!(
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|source| source.id() == use_id),
                "each resource or text use resolves in the snapshot inventory"
            );
        }
    }
    Ok(())
}

#[test]
fn no_element_resource_link_produces_no_resource_names() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = archive(&folder.path().join("Data.p4k"), false)?;
    let snapshot = read_mining_locations(
        &archive,
        MiningLocationOptions::new(Language::English, None),
    )?;
    assert_eq!(
        snapshot.mineable_elements.len(),
        1,
        "the element is still available"
    );
    assert_eq!(
        snapshot.resource_types.len(),
        0,
        "no resource Records are reached"
    );
    assert_eq!(
        snapshot.public_names.len(),
        0,
        "no resource names are reached"
    );
    assert_eq!(
        snapshot.coverage.unreadable_resource_types, 0,
        "all requested resource rows are readable"
    );
    Ok(())
}

fn archive(path: &Path, linked: bool) -> TestResult<Archive> {
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dataforge(linked)))
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"ore=Fixture ore\nrefined=Fixture refined\nunreached=Unreached resource\n",
        ))
        .write_to(path)?;
    Ok(Archive::open(path)?)
}

fn dataforge(linked: bool) -> Vec<u8> {
    let resource = TestStruct::new("ResourceType")
        .field("displayName", TestType::Locale)
        .field(
            "refinedVersion",
            TestType::ReferenceTo("ResourceType".to_owned()),
        );
    let element = TestStruct::new("MineableElement").field(
        "resourceType",
        TestType::ReferenceTo("ResourceType".to_owned()),
    );
    let mut source = TestRecord::new(
        ELEMENT,
        "MineableElement",
        "MineableElement.Fixture",
        "libs/foundry/records/mining/mineableelements/fixture.xml",
    );
    if linked {
        source = source.value("resourceType", TestValue::Reference(ORE));
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(resource)
        .structure(element)
        .record(resource_record(ORE, "ore", Some(REFINED)))
        .record(resource_record(REFINED, "refined", Some(UNREACHED)))
        .record(resource_record(UNREACHED, "unreached", None))
        .record(source)
        .build()
}

fn resource_record(guid: u128, name: &str, refined: Option<u128>) -> TestRecord {
    let record = TestRecord::new(
        guid,
        "ResourceType",
        format!("ResourceType.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{name}.xml"),
    )
    .value("displayName", TestValue::text(format!("@{name}")));
    if let Some(refined) = refined {
        record.value("refinedVersion", TestValue::Reference(refined))
    } else {
        record
    }
}
