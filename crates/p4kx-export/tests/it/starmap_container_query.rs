// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::StarmapTraversalBudget;
use p4kx_export::catalog_queries::starmap::{
    ContainerInstanceKey, ContainerPlacementSelection, ContainerPlacementSnapshot,
    ContainerRootStatus, ContainerTargetReadStatus, ContainerTargetStatus,
    read_container_placements,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::{Map, Value, json};

#[path = "starmap_location_physics_query.rs"]
mod location_physics;

use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const ROOT: &str = r"Data\ObjectContainers\PU\system\alpha\alphasystem.socpak";
const UNRELATED: &str = r"Data\ObjectContainers\PU\system\other\othersystem.socpak";
const TILE: &str = r"Data\ObjectContainers\PU\alpha\tile.socpak";
const BROKEN: &str = r"Data\ObjectContainers\PU\alpha\broken.socpak";

#[test]
fn scoped_container_rows_match_both_canonical_tables_and_skip_unrelated_roots() -> TestResult {
    let source = fixture(true)?;
    let root = entry_index(&source.archive, ROOT)?;
    let (_, catalog) = source.diagnostic()?;
    let selected = read_root(&source.archive, root)?;
    assert_unavailable_dependency(&selected);
    let [unresolved] = selected.unresolved_container_links.as_slice() else {
        return Err("one unavailable original Child target".into());
    };
    assert!(
        matches!(
            unresolved.target_status.value(),
            Some(ContainerTargetStatus::Resolved)
        ),
        "directory selection retained the unique original target"
    );
    assert!(
        matches!(
            unresolved.target_read_status.value(),
            Some(ContainerTargetReadStatus::Unreadable)
        ),
        "the unique target body could not be opened"
    );
    let key = unresolved
        .instance_id
        .value()
        .ok_or("original unresolved occurrence")?;
    let exact = read_occurrence(&source.archive, key)?;
    assert!(
        exact.container_instances.is_empty(),
        "the selected unresolved Child has no resolved occurrence"
    );
    let [row] = exact.unresolved_container_links.as_slice() else {
        return Err("exactly the selected unresolved occurrence".into());
    };
    assert_eq!(
        row.instance_id.value(),
        Some(key),
        "the unresolved row retains its exact route"
    );
    let scoped = document(&selected);
    assert_no_entry(&scoped, UNRELATED)?;
    for table in ["containerInstances", "unresolvedContainerLinks"] {
        let path = format!("/placements/{table}/rows");
        assert_table(&catalog, &scoped, &path, root)?;
    }
    proof::closure(&scoped)
}

#[test]
fn an_exact_occurrence_keeps_the_whole_route_and_selected_system_health() -> TestResult {
    let source = fixture(false)?;
    let root = entry_index(&source.archive, ROOT)?;
    let initial = read_root(&source.archive, root)?;
    assert_complete_system(&initial);
    let last = initial
        .container_instances
        .last()
        .and_then(|row| row.instance_id.value())
        .ok_or("last original tile occurrence")?;
    let selected = read_occurrence(&source.archive, last)?;
    let [tile] = selected.container_instances.as_slice() else {
        return Err("exactly one selected root-to-leaf occurrence".into());
    };
    assert_eq!(
        tile.instance_id.value(),
        Some(last),
        "the complete repeated-tile route identifies one occurrence"
    );
    assert_eq!(
        tile.world_position.value(),
        Some(&[7.0, 0.0, 0.0]),
        "the selected occurrence carries its own composed transform"
    );
    assert!(
        selected.unresolved_container_links.is_empty(),
        "the selected tile remains resolved"
    );
    assert_complete_system(&selected);
    let mut longer = vec![u32::MAX];
    longer.extend_from_slice(last.child_node_indices());
    let impossible = read_container_placements(
        &source.archive,
        ContainerPlacementSelection::Occurrence {
            root_entry_index: root,
            child_node_indices: &longer,
        },
        StarmapTraversalBudget::new(),
        None,
    )?;
    assert!(
        impossible.container_instances.is_empty()
            && impossible.unresolved_container_links.is_empty(),
        "a longer route ending at the same Child XML ordinal matches neither table"
    );
    assert!(
        impossible.coverage.is_complete(),
        "the requested graph was completely checked"
    );
    let non_root = read_root(&source.archive, entry_index(&source.archive, TILE)?)?;
    assert_eq!(
        non_root.coverage.root,
        ContainerRootStatus::Absent,
        "the tile is outside canonical root selection"
    );
    assert_eq!(
        non_root.coverage.attempted_descriptors, 0,
        "non-root selection ends at directory admission"
    );
    Ok(())
}

fn assert_unavailable_dependency(snapshot: &ContainerPlacementSnapshot) {
    let coverage = &snapshot.coverage;
    assert_eq!(
        coverage.root,
        ContainerRootStatus::Selected,
        "the original system root is admitted"
    );
    assert!(
        !coverage.is_complete(),
        "the unreadable selected target blocks completeness"
    );
    assert_eq!(
        coverage.attempted_descriptors, 3,
        "the system root and both reached physical targets are attempted once"
    );
    assert_eq!(
        coverage.unreadable_containers, 1,
        "only the reached broken target is counted"
    );
    assert_eq!(
        coverage.source_entries.failed_dependencies, 1,
        "Entry health retains the selected target failure"
    );
    assert_eq!(
        snapshot.container_instances.len(),
        3,
        "the root and both occurrences of the valid tile are retained"
    );
}

fn assert_complete_system(snapshot: &ContainerPlacementSnapshot) {
    assert!(
        snapshot.coverage.is_complete(),
        "the selected system has a complete traversal"
    );
    assert_eq!(
        snapshot.coverage.attempted_descriptors, 2,
        "the root and repeated tile are the only distinct descriptor reads"
    );
}

fn read_root(archive: &Archive, root: u32) -> TestResult<ContainerPlacementSnapshot> {
    Ok(read_container_placements(
        archive,
        ContainerPlacementSelection::SystemRoot(root),
        StarmapTraversalBudget::new(),
        None,
    )?)
}

fn read_occurrence(
    archive: &Archive,
    key: &ContainerInstanceKey,
) -> TestResult<ContainerPlacementSnapshot> {
    let selected = read_container_placements(
        archive,
        ContainerPlacementSelection::ExactOccurrence {
            root_entry_index: key.root_entry_index(),
            child_node_indices: key.child_node_indices(),
            external_child_step_indices: key.external_child_step_indices(),
        },
        StarmapTraversalBudget::new(),
        None,
    )?;
    assert_eq!(
        selected.external_child_step_indices.as_deref(),
        Some(key.external_child_step_indices()),
        "the snapshot preserves the exact selector including an empty step list"
    );
    Ok(selected)
}

fn entry_index(archive: &Archive, path: &str) -> TestResult<u32> {
    let original = crate::health_origin_entry_source::original(archive, path)?;
    Ok(u32::try_from(original.index())?)
}

fn document(snapshot: &ContainerPlacementSnapshot) -> Value {
    json!({
        "placements": {
            "containerInstances": { "rows": snapshot.container_instances },
            "unresolvedContainerLinks": { "rows": snapshot.unresolved_container_links },
        },
        "provenance": snapshot.source_inventory,
    })
}

fn assert_no_entry(scoped: &Value, path: &str) -> TestResult {
    let sources = proof::rows(scoped, "/provenance/sources")?;
    assert!(
        sources
            .iter()
            .all(|source| source.pointer("/path/value") != Some(&json!(path))),
        "the unrelated root supplies no Entry evidence"
    );
    Ok(())
}

fn assert_table(catalog: &Value, scoped: &Value, path: &str, root: u32) -> TestResult {
    let present = proof::rows(catalog, path)?
        .iter()
        .filter(|row| row.pointer("/instanceId/value/rootEntryIndex") == Some(&json!(root)))
        .map(|row| normalized(row, catalog))
        .collect::<Result<Vec<_>, _>>()?;
    let actual = proof::rows(scoped, path)?
        .iter()
        .map(|row| normalized(row, scoped))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        actual, present,
        "original rows and physical use coordinates in {path}"
    );
    Ok(())
}

fn normalized(value: &Value, document: &Value) -> TestResult<Value> {
    match value {
        Value::Object(object) => {
            let fields = object
                .iter()
                .map(|(key, field)| {
                    let field = match key.as_str() {
                        "sourceUsesRefs" => normalize_uses(value, document)?,
                        "inputRef" => physical_source(field, document)?,
                        _ => normalized(field, document)?,
                    };
                    Ok((key.clone(), field))
                })
                .collect::<TestResult<Map<String, Value>>>()?;
            Ok(Value::Object(fields))
        }
        Value::Array(array) => Ok(Value::Array(
            array
                .iter()
                .map(|entry| normalized(entry, document))
                .collect::<TestResult<Vec<_>>>()?,
        )),
        _ => Ok(value.clone()),
    }
}

fn normalize_uses(carrier: &Value, document: &Value) -> TestResult<Value> {
    let mut uses = proof::uses(document, carrier)?
        .into_iter()
        .map(|usage| {
            Ok(json!({
                "source": physical_source(proof::at(usage, "/source")?, document)?,
                "scope": proof::at(usage, "/scope")?,
                "role": proof::at(usage, "/role")?,
                "location": proof::at(usage, "/location")?,
            }))
        })
        .collect::<TestResult<Vec<_>>>()?;
    uses.sort_by_cached_key(Value::to_string);
    Ok(Value::Array(uses))
}

fn physical_source(reference: &Value, document: &Value) -> TestResult<Value> {
    let source = proof::rows(document, "/provenance/sources")?
        .iter()
        .find(|entry| entry.get("id") == Some(reference))
        .ok_or("original physical source in result inventory")?;
    let ancestors = proof::rows(source, "/ancestors")?
        .iter()
        .map(|reference| physical_source(reference, document))
        .collect::<TestResult<Vec<_>>>()?;
    Ok(json!({
        "kind": proof::at(source, "/kind")?,
        "entryIndex": source.get("entryIndex"),
        "path": source.pointer("/path/value"),
        "storedEncryption": proof::at(source, "/storedEncryption")?,
        "ancestors": ancestors,
    }))
}

fn fixture(broken_child: bool) -> TestResult<Fixture> {
    let mut children = TestElement::new("ChildObjectContainers")
        .child(child(TILE, "2,0,0"))
        .child(child(TILE, "7,0,0"));
    if broken_child {
        children = children.child(child(BROKEN, "4,0,0"));
    }
    let root = TestElement::new("ObjectContainer")
        .attribute("version", "14")
        .child(children)
        .to_xml();
    let mut entries = vec![
        packed(ROOT, "alphasystem.xml", root.as_bytes()).encrypted(),
        packed(TILE, "tile.xml", b"<ObjectContainer/>"),
        TestEntry::stored(UNRELATED, b"not a readable container".as_slice()),
    ];
    if broken_child {
        entries.push(TestEntry::stored(
            BROKEN,
            b"not a readable target".as_slice(),
        ));
    }
    Fixture::open(&fixture::empty_database(), Input::Plain, entries)
}

fn child(path: &str, position: &str) -> TestElement {
    TestElement::new("Child")
        .attribute("external", "true")
        .attribute("name", path)
        .attribute("pos", position)
        .attribute("rot", "1,0,0,0")
}

fn packed(path: &str, file: &str, xml: &[u8]) -> TestEntry {
    let container = TestContainer::new()
        .entry(TestEntry::stored(file, xml))
        .build();
    TestEntry::stored(path, container)
}
