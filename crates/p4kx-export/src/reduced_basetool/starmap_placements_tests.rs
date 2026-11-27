// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::jump_inventory;
use super::record_targets::RecordTargets;
use super::starmap_placements::{self, PATH};
use crate::StarmapTraversalBudget;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_queries::starmap::{
    self, StarmapDefinitionFamily, StarmapDefinitionRows, StarmapDefinitionSelection,
};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};

const ROOT: &str = "Data/ObjectContainers/PU/system/fixture/fixturesystem.socpak";
const OBJECT: &str = "StarMapObject";
const TYPE: &str = "StarMapObjectType";
const JUMP_TYPE: u128 = 0x1177_9200;
const PARENT: u128 = 0x1177_9201;
const CHILD: u128 = 0x1177_9202;
const UNPLACED: u128 = 0x1177_9203;
const AMBIGUOUS: u128 = 0x1177_9204;
const JUMP: u128 = 0x1177_9205;
const UNKNOWN: u128 = 0x1177_9206;

#[test]
fn reduced_starmap_placements_share_jump_inventory_and_keep_bound_original_coordinates()
-> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = open(scratch.path(), descriptor())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let owners = targets(&parsed, None)?;
    let reduced = starmap_placements::collect(
        &parsed,
        &owners,
        StarmapTraversalBudget::new(),
        scratch.path(),
        None,
    )?;
    let json = serde_json::to_value(reduced.placements)?;
    let Some([parent, child, jump]) = json
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Err("only three bound Object Records".into());
    };
    for (record, row) in [(1, parent), (2, child), (5, jump)] {
        compare_canonical(&archive, record, row)?;
    }
    assert_eq!(
        child.pointer("/position/value"),
        Some(&json!([101.235, 202.5, 303.25])),
        "system coordinates include the ancestor offset and canonical millimetre rounding"
    );
    assert_eq!(
        child.pointer("/placedUnderGuid/value"),
        parent.pointer("/objectGuid/value"),
        "the exact nearest placed ancestor"
    );
    assert_eq!(
        child.pointer("/entityName/value"),
        Some(&json!("fixture_child")),
        "the original Child entity name"
    );
    assert_eq!(
        jump.pointer("/placedUnderGuid/value"),
        Some(&json!(format!("00000000-0000-0000-0000-{UNKNOWN:012x}"))),
        "the nearest coordinate-frame GUID remains even without a catalog definition"
    );
    compare_jump_inventory(&archive, scratch.path(), reduced.jumps)?;
    reject_missing_endpoint_object(&parsed, scratch.path())
}

#[test]
fn reduced_starmap_placements_refuse_incomplete_system_traversal() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = open(scratch.path(), b"unreadable container".to_vec())?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    assert!(
        matches!(
            starmap_placements::collect(
                &parsed,
                &targets(&parsed, None)?,
                StarmapTraversalBudget::new(),
                scratch.path(),
                None,
            ),
            Err(ReducedError::IncompleteScope(PATH))
        ),
        "an unreadable admitted root cannot certify an empty bound-placement table"
    );
    Ok(())
}

fn compare_canonical(archive: &Archive, index: u32, row: &Value) -> TestResult {
    let snapshot = starmap::read_starmap_object_placement(
        archive,
        index,
        StarmapTraversalBudget::new(),
        None,
    )?;
    let [canonical] = snapshot.rows.as_slice() else {
        return Err("one physical canonical placement".into());
    };
    let canonical = serde_json::to_value(canonical)?;
    for field in [
        "objectGuid",
        "objectRecordIndex",
        "system",
        "entityName",
        "placedUnderGuid",
        "position",
    ] {
        let selected = row.get(field).ok_or("selected reduced cell")?;
        let original = canonical.get(field).ok_or("original placement cell")?;
        for fact in ["state", "value", "containsEncryptedSource"] {
            assert_eq!(
                selected.get(fact),
                original.get(fact),
                "original {field}.{fact} at physical Record {index}"
            );
        }
        assert!(
            selected.get("sourceUsesRefs").is_none(),
            "no source-use arena at {field}"
        );
    }
    Ok(())
}

fn compare_jump_inventory(
    archive: &Archive,
    scratch: &Path,
    actual: jump_inventory::JumpInventory,
) -> TestResult {
    let canonical = starmap::read_jump_inventory(archive, StarmapTraversalBudget::new(), None)?;
    assert_eq!(
        canonical.endpoints.len(),
        1,
        "one original local Jump endpoint"
    );
    let expected = jump_inventory::stage(
        canonical.endpoint_availability,
        &canonical.endpoints,
        &canonical.connections,
        scratch,
        None,
    )?;
    assert_eq!(
        serde_json::to_value(actual.endpoints)?,
        serde_json::to_value(expected.endpoints)?,
        "the combined and standalone endpoint rows agree"
    );
    assert_eq!(
        serde_json::to_value(actual.connections)?,
        serde_json::to_value(expected.connections)?,
        "the same surveyed empty connection inventory"
    );
    Ok(())
}

fn reject_missing_endpoint_object(parsed: &ParsedCatalogQueries<'_>, scratch: &Path) -> TestResult {
    assert!(
        matches!(
            starmap_placements::collect(
                parsed,
                &targets(parsed, Some(5))?,
                StarmapTraversalBudget::new(),
                scratch,
                None,
            ),
            Err(ReducedError::UnresolvedRecordLink {
                path: jump_inventory::ENDPOINTS,
                index: 5
            })
        ),
        "the actual jump endpoint requires its exact staged Object Record"
    );
    Ok(())
}

fn targets(parsed: &ParsedCatalogQueries<'_>, exclude: Option<u32>) -> TestResult<RecordTargets> {
    let selection = StarmapDefinitionSelection::new(
        StarmapDefinitionFamily::Objects,
        CatalogRecordSelection::All,
    );
    parsed.with_starmap_definitions(selection, None, |snapshot| {
        let StarmapDefinitionRows::Objects(rows) = &snapshot.rows else {
            return Err("canonical Object Record list".into());
        };
        let mut physical = RecordTargets::default();
        for row in rows {
            if row.source.record_index().value().copied() != exclude || exclude.is_none() {
                physical.insert("definitions.starmapObjects", &row.source)?;
            }
        }
        Ok(physical)
    })?
}

fn open(folder: &Path, system: Vec<u8>) -> TestResult<Archive> {
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .entry(TestEntry::stored(ROOT, system).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

fn database() -> DataForgeBuilder {
    let schema = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(TYPE).field("name", TestType::String))
        .structure(TestStruct::new(OBJECT).field("type", TestType::ReferenceTo(TYPE.to_owned())))
        .record(
            TestRecord::new(
                JUMP_TYPE,
                TYPE,
                "StarMapObjectType.FixtureJump",
                "fixture/jump_type.xml",
            )
            .value("name", TestValue::text("JumpPoint")),
        );
    [
        (PARENT, "parent"),
        (CHILD, "child"),
        (UNPLACED, "unplaced"),
        (AMBIGUOUS, "ambiguous"),
        (JUMP, "jump"),
    ]
    .into_iter()
    .fold(schema, |builder, (id, name)| {
        let record = TestRecord::new(
            id,
            OBJECT,
            format!("{OBJECT}.{name}"),
            format!("libs/foundry/records/starmap/pu/system/fixture/{name}.xml"),
        );
        builder.record(if id == JUMP {
            record.value("type", TestValue::Reference(JUMP_TYPE))
        } else {
            record
        })
    })
}

fn descriptor() -> Vec<u8> {
    let children = format!(
        r#"<ObjectContainer><ChildObjectContainers>
        <Child pos="100,200,300" entityName="fixture_parent" starMapRecord="00000000-0000-0000-0000-{PARENT:012x}">
            <ChildObjectContainers>
                <Child pos="1.23456,2.5,3.25" entityName="fixture_child" starMapRecord="00000000-0000-0000-0000-{CHILD:012x}"/>
            </ChildObjectContainers>
        </Child>
        <Child pos="4,5,6" starMapRecord="00000000-0000-0000-0000-{AMBIGUOUS:012x}"/>
        <Child pos="7,8,9" starMapRecord="00000000-0000-0000-0000-{AMBIGUOUS:012x}"/>
        <Child pos="10,20,30" starMapRecord="00000000-0000-0000-0000-{UNKNOWN:012x}">
            <ChildObjectContainers>
                <Child pos="1,2,3" entityName="fixture_jump" starMapRecord="00000000-0000-0000-0000-{JUMP:012x}"/>
            </ChildObjectContainers>
        </Child>
    </ChildObjectContainers></ObjectContainer>"#
    );
    TestContainer::new()
        .entry(TestEntry::stored(
            "fixturesystem.xml",
            children.into_bytes(),
        ))
        .build()
}
