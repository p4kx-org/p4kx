// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_extraction::{Archive, FieldOrigin, Filter, Language, SourceInventory};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

use crate::mining_locations::{MiningLocationOptions, MiningStarmapObject, read_mining_locations};
use crate::{CatalogOptions, build_catalog};

const AUTHORED_PARENT: u128 = 0xe001;
const PHYSICAL_PARENT: u128 = 0xe002;
const CHILD: u128 = 0xe003;
const PHYSICAL_GUID: &str = "00000000-0000-0000-0000-00000000e002";
const CHILD_GUID: &str = "00000000-0000-0000-0000-00000000e003";

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn authored_and_placement_parents_match_their_distinct_catalog_sources() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = archive(&folder.path().join("Data.p4k"))?;
    let snapshot = read_mining_locations(
        &archive,
        MiningLocationOptions::new(Language::English, None),
    )?;
    let filter = Filter::parse("*/starmap/pu/*")?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.filter = Some(&filter);
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let child = snapshot
        .starmap_objects
        .iter()
        .find(|row| row.source.record_index.value() == Some(&2))
        .ok_or("physical child")?;
    compare_parents(child, &catalog)?;
    check_origin(
        &snapshot.source_inventory,
        [child.parent.origin(), child.placed_under_guid.origin()],
    );
    Ok(())
}

fn compare_parents(child: &MiningStarmapObject, catalog: &Value) -> TestResult {
    let definitions = catalog
        .pointer("/definitions/starmapObjects/rows")
        .and_then(Value::as_array)
        .ok_or("authored definitions")?;
    let original = definitions
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(CHILD_GUID))
        .ok_or("child definition")?;
    let placements = catalog
        .pointer("/placements/starmapPlacements/rows")
        .and_then(Value::as_array)
        .ok_or("placements")?;
    let placed = placements
        .iter()
        .find(|row| row.pointer("/objectGuid/value").and_then(Value::as_str) == Some(CHILD_GUID))
        .ok_or("child placement")?;
    assert_eq!(
        &serde_json::to_value(&child.parent)?,
        &original["parent"],
        "authored relationship equals the Catalog definition"
    );
    let parent = child.parent.value().ok_or("authored parent")?;
    assert_eq!(
        parent.target_record_index().value(),
        Some(&0),
        "authored parent joins by original Record index"
    );
    let physical = child
        .placed_under_guid
        .value()
        .ok_or("physical enclosing Record")?;
    assert_eq!(
        physical.to_string(),
        PHYSICAL_GUID,
        "the SDK retains the physical container parent"
    );
    assert_eq!(
        placed
            .pointer("/placedUnderGuid/value")
            .and_then(Value::as_str),
        Some(PHYSICAL_GUID),
        "the Catalog placement agrees with the SDK physical relationship"
    );
    assert_ne!(
        parent.guid().value(),
        Some(physical),
        "the fixture proves the two origins do not alias"
    );
    Ok(())
}

fn check_origin(inventory: &SourceInventory, origins: [&FieldOrigin; 2]) {
    for origin in origins {
        assert_ne!(
            origin.source_uses_refs(),
            [],
            "each relationship has evidence"
        );
        for use_id in origin.source_uses_refs() {
            assert!(
                inventory.iter_uses().any(|source| source.id() == use_id),
                "each relationship use belongs to the returned source inventory"
            );
        }
    }
}

fn archive(path: &Path) -> TestResult<Archive> {
    let root = format!(
        r#"<ObjectContainer><ChildObjectContainers>
        <Child starMapRecord="{PHYSICAL_GUID}" pos="10,0,0"><ChildObjectContainers>
        <Child starMapRecord="{CHILD_GUID}" pos="1,0,0"/>
        </ChildObjectContainers></Child>
        </ChildObjectContainers></ObjectContainer>"#,
    );
    let container = TestContainer::new()
        .entry(TestEntry::stored("tsystem.xml", root))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dataforge()))
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"body=Fixture body\n",
        ))
        .entry(TestEntry::stored(
            "Data/ObjectContainers/PU/system/t/tsystem.socpak",
            container,
        ))
        .write_to(path)?;
    Ok(Archive::open(path)?)
}

fn dataforge() -> Vec<u8> {
    let object = TestStruct::new("StarMapObject")
        .field("name", TestType::Locale)
        .field("parent", TestType::ReferenceTo("StarMapObject".to_owned()));
    DataForgeBuilder::new()
        .version(8)
        .structure(object)
        .record(record(AUTHORED_PARENT, "authored"))
        .record(record(PHYSICAL_PARENT, "physical"))
        .record(record(CHILD, "child").value("parent", TestValue::Reference(AUTHORED_PARENT)))
        .build()
}

fn record(id: u128, name: &str) -> TestRecord {
    TestRecord::new(
        id,
        "StarMapObject",
        format!("StarMapObject.{name}"),
        format!("libs/foundry/records/starmap/pu/{name}.xml"),
    )
    .value("name", TestValue::text("@body"))
}
