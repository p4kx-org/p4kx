// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog, catalog_schema};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::{EXPORTED_AT, TestResult};
use crate::health_origin_assertions as proof;
use crate::health_origin_entry_source as entries;

const ROOT: &str = "Data/ObjectContainers/PU/system/canary/canarysystem.socpak";
const TILE: &str = "Data/ObjectContainers/PU/Starchitect/Tile/instance.socpak";
const RECORD: u128 = 0x844_0101;
const LABEL_KEY: &str = "@sample_label";
const MISSING: &str = "Data/ObjectContainers/PU/Starchitect/Tile/missing.socpak";

#[test]
fn one_tile_file_keeps_two_physical_instances_and_their_own_transforms() -> TestResult {
    let catalog = catalog(None, true, LABEL_KEY)?;
    let rows = proof::rows(&catalog, "/placements/containerInstances/rows")?;
    let root = by_path(rows, ROOT)
        .next()
        .ok_or("root container instance")?;
    let mut tiles = by_path(rows, TILE);
    let rotated = tiles.next().ok_or("first tile instance")?;
    let direct = tiles.next().ok_or("second tile instance")?;
    assert!(
        tiles.next().is_none(),
        "one row per actual traversal occurrence"
    );
    assert_instance_identity(root, rotated, direct)?;
    proof::present(
        rotated,
        "localPosition",
        &json!([2.0, 0.0, 0.0]),
        Some(true),
    )?;
    let local_rotation = proof::present(
        rotated,
        "localRotation",
        &json!([2.0, 0.0, 0.0, 0.0]),
        Some(true),
    )?;
    let world_position = proof::present(
        rotated,
        "worldPosition",
        &json!([10.0, 2.0, 0.0]),
        Some(true),
    )?;
    proof::present(
        direct,
        "worldPosition",
        &json!([100.0, 0.0, 0.0]),
        Some(true),
    )?;
    let missing_rotation = proof::cell(direct, "localRotation", "absent")?;
    assert_eq!(
        proof::at(missing_rotation, "/containsEncryptedSource")?,
        true
    );
    assert!(
        by_path(rows, MISSING).next().is_none(),
        "an unresolved target has no occurrence"
    );
    assert_component_origins(&catalog, rotated, local_rotation, world_position)?;
    let validator = jsonschema::validator_for(&catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "the runtime schema accepts the actual rows"
    );
    proof::closure(&catalog)
}

fn assert_instance_identity(root: &Value, rotated: &Value, direct: &Value) -> TestResult {
    assert_ne!(
        proof::at(rotated, "/instanceId/value")?,
        proof::at(direct, "/instanceId/value")?,
        "reused source entries retain distinct instance identity"
    );
    assert_eq!(
        proof::at(rotated, "/container/value")?,
        proof::at(direct, "/container/value")?,
        "both routes select the one original file"
    );
    for tile in [rotated, direct] {
        assert_eq!(
            proof::at(tile, "/parentInstanceId/value")?,
            proof::at(root, "/instanceId/value")?,
            "inline children retain their enclosing physical parent instance"
        );
        proof::present(tile, "declaredPath", &json!(TILE), Some(true))?;
    }
    Ok(())
}

fn assert_component_origins(
    catalog: &Value,
    occurrence: &Value,
    local_rotation: &Value,
    world_position: &Value,
) -> TestResult {
    let leaf = proof::rows(occurrence, "/instanceId/value/childNodeIndices")?
        .last()
        .and_then(Value::as_u64)
        .ok_or("instance terminal Child")?;
    let local = proof::uses(catalog, local_rotation)?;
    let [attribute] = local.as_slice() else {
        return Err("the authored quaternion has one original attribute".into());
    };
    assert_eq!(
        proof::at(attribute, "/location/field")?,
        "@rot",
        "the rotation comes from its declared slot"
    );
    assert_eq!(
        original_node(attribute)?,
        leaf,
        "the original local quaternion belongs to the terminal Child"
    );
    let world = proof::uses(catalog, world_position)?;
    let rotations: Vec<_> = world
        .into_iter()
        .filter(|used| used.pointer("/location/field").and_then(Value::as_str) == Some("@rot"))
        .collect();
    let [ancestor] = rotations.as_slice() else {
        return Err("only the ancestor rotation acts on the translation".into());
    };
    assert_ne!(
        original_node(ancestor)?,
        leaf,
        "the terminal rotation does not act on its translation"
    );
    Ok(())
}

fn original_node(used: &Value) -> Result<u64, Box<dyn std::error::Error>> {
    Ok(proof::at(used, "/location/indices/0")?
        .as_str()
        .ok_or("exact XML node coordinate")?
        .parse()?)
}

#[test]
fn reused_instances_keep_their_own_authored_child_labels() -> TestResult {
    for label in [LABEL_KEY, "Synthetic display label"] {
        assert_instance_labels(&catalog(None, true, label)?, label)?;
    }
    Ok(())
}

fn assert_instance_labels(catalog: &Value, first_label: &str) -> TestResult {
    let rows = proof::rows(catalog, "/placements/containerInstances/rows")?;
    let root = by_path(rows, ROOT).next().ok_or("root occurrence")?;
    proof::cell(root, "declaredLabel", "absent")?;
    proof::cell(root, "declaredEntityName", "absent")?;

    let mut instances = by_path(rows, TILE);
    let first = instances.next().ok_or("first tile occurrence")?;
    let second = instances.next().ok_or("second tile occurrence")?;
    let label = proof::present(first, "declaredLabel", &json!(first_label), Some(true))?;
    let entity = proof::present(
        first,
        "declaredEntityName",
        &json!("SampleEntity"),
        Some(true),
    )?;
    assert_child_attribute_source(catalog, first, label, "@label")?;
    assert_child_attribute_source(catalog, first, entity, "@entityName")?;

    let empty = proof::present(second, "declaredLabel", &json!(""), Some(true))?;
    let missing = proof::cell(second, "declaredEntityName", "absent")?;
    assert_eq!(
        proof::at(missing, "/containsEncryptedSource")?,
        true,
        "the missing attribute is observed in the encrypted Child XML"
    );
    assert_eq!(
        proof::at(missing, "/sourceSetComplete")?,
        true,
        "the complete original attribute slot was inspected"
    );
    assert_child_attribute_source(catalog, second, empty, "@label")?;
    assert_child_attribute_source(catalog, second, missing, "$absent:@entityName")?;
    proof::closure(catalog)
}

fn assert_child_attribute_source(
    catalog: &Value,
    instance: &Value,
    field: &Value,
    attribute: &str,
) -> TestResult {
    let leaf = proof::rows(instance, "/instanceId/value/childNodeIndices")?
        .last()
        .and_then(Value::as_u64)
        .ok_or("the declaring Child node ordinal")?;
    let uses = proof::uses(catalog, field)?;
    let [source] = uses.as_slice() else {
        return Err("a direct authored attribute needs its one exact source use".into());
    };
    assert_eq!(
        proof::at(source, "/location/field")?,
        attribute,
        "the source is the exact declared or observed-absent Child attribute"
    );
    assert_eq!(
        original_node(source)?,
        leaf,
        "the label is sourced from this occurrence's terminal Child"
    );
    Ok(())
}

#[test]
fn unrelated_record_filter_leaves_physical_container_inventory_unavailable() -> TestResult {
    let filter = Filter::new("*/unrelated/*");
    let catalog = catalog(Some(&filter), true, LABEL_KEY)?;
    assert_eq!(
        proof::at(&catalog, "/placements/containerInstances/availability")?,
        "unavailable"
    );
    assert_eq!(
        proof::rows(&catalog, "/placements/containerInstances/rows")?,
        &[] as &[Value],
        "a filter outside placement records does not request the archive inventory"
    );
    Ok(())
}

fn by_path<'catalog>(
    rows: &'catalog [Value],
    path: &'static str,
) -> impl Iterator<Item = &'catalog Value> {
    rows.iter()
        .filter(move |row| row.pointer("/sourcePath/value").and_then(Value::as_str) == Some(path))
}

#[test]
fn an_unfiltered_catalog_discovers_file_instances_without_matching_records() -> TestResult {
    let catalog = catalog(None, false, LABEL_KEY)?;
    assert_eq!(
        proof::rows(&catalog, "/definitions/starmapObjects/rows")?,
        &[] as &[Value],
        "the fixture contributes no DataForge placement records"
    );
    assert_eq!(
        proof::at(&catalog, "/placements/containerInstances/availability")?,
        "available"
    );
    assert_eq!(
        proof::rows(&catalog, "/placements/containerInstances/rows")?.len(),
        3
    );
    assert_eq!(
        entries::failure(&catalog, "starmapObjects", "enumeration")?.len(),
        1
    );
    assert_eq!(
        entries::failure(&catalog, "lootContainers", "enumeration")?,
        [] as [&Value; 0],
        "the file-only walk does not add a Loot dependency"
    );
    Ok(())
}

fn catalog(
    filter: Option<&Filter>,
    with_starmap_record: bool,
    first_label: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let root = format!(
        r#"<ObjectContainer version="14">
          <ChildObjectContainers>
            <Child pos="10,0,0" rot="0.7071067811865475,0,0,0.7071067811865475">
              <ChildObjectContainers><Child external="1" name="{TILE}" pos="2,0,0" rot="2,0,0,0" label="{first_label}" entityName="SampleEntity"/></ChildObjectContainers>
            </Child>
          </ChildObjectContainers>
          <ChildObjectContainers>
            <Child external="1" name="{TILE}" pos="100,0,0" label=""/>
            <Child external="1" name="{MISSING}"/>
          </ChildObjectContainers>
        </ObjectContainer>"#,
    );
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            dataforge(with_starmap_record),
        ))
        .entry(TestEntry::stored(ROOT, socpak("canarysystem.xml", root)).encrypted())
        .entry(TestEntry::stored(
            TILE,
            socpak("instance.xml", r#"<ObjectContainer version="14"/>"#),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        filter,
        ..CatalogOptions::new(std::time::UNIX_EPOCH + EXPORTED_AT)
    };
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn dataforge(with_starmap_record: bool) -> Vec<u8> {
    let mut forge = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObject").field("hideInStarmap", TestType::Bool));
    if with_starmap_record {
        forge = forge.record(
            TestRecord::new(
                RECORD,
                "StarMapObject",
                "StarMapObject.InstanceAnchor",
                "libs/foundry/records/starmap/pu/system/canary/anchor.xml",
            )
            .value("hideInStarmap", TestValue::Bool(false)),
        );
    }
    forge.build()
}

fn socpak(name: &str, xml: impl Into<Vec<u8>>) -> Vec<u8> {
    TestContainer::new()
        .entry(TestEntry::stored(name, xml))
        .entry(TestEntry::stored("empty.soc", Vec::new()))
        .build()
}
