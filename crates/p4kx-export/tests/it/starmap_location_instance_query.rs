// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::StarmapTraversalBudget;
use p4kx_export::catalog_queries::starmap;
use p4kx_export::catalog_queries::starmap::{
    ContainerPlacementSelection, ContainerRootStatus, LocationInstance, LocationPlacementSnapshot,
};
use p4kx_testkit::{TestContainer, TestEntry};
use serde_json::json;

use super::{ROOT, TILE, UNRELATED, assert_no_entry, assert_table, entry_index};
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const GUID: &str = "10000000-0000-4000-8000-000000000001";
const PRESET: &str = "10000000-0000-4000-8000-000000000002";
const TABLE: &str = "/placements/locationInstances/rows";

#[test]
fn location_instances_share_rows_and_source_coordinates_with_the_catalog() -> TestResult {
    let input = source()?;
    let root = entry_index(&input.archive, ROOT)?;
    let catalog = input.diagnostic()?.1;
    let selected = starmap::read_location_instances(
        &input.archive,
        ContainerPlacementSelection::SystemRoot(root),
        StarmapTraversalBudget::new(),
        None,
    )?;
    assert!(
        selected.coverage.is_complete(),
        "both selected descriptors and their companions are available"
    );
    assert_eq!(
        selected.coverage.attempted_descriptors, 2,
        "the reused tile is decoded once for the selected root"
    );
    let document = json!({
        "placements": { "locationInstances": { "rows": selected.location_instances } },
        "provenance": selected.source_inventory,
    });
    assert_table(&catalog, &document, TABLE, root)?;
    assert_no_entry(&document, UNRELATED)?;
    assert_no_entry(&document, "Data\\Game2.dcb")?;
    proof::closure(&document)
}

#[test]
fn exact_location_keeps_its_parent_and_authored_class_and_companions() -> TestResult {
    let input = source()?;
    let root = entry_index(&input.archive, ROOT)?;
    let all = starmap::read_location_instances(
        &input.archive,
        ContainerPlacementSelection::SystemRoot(root),
        StarmapTraversalBudget::new(),
        None,
    )?;
    let [first_parent, first_child, second_parent, second_child] =
        all.location_instances.as_slice()
    else {
        return Err("two physical uses of one typed tile Child".into());
    };
    let first = first_child
        .instance_id()
        .value()
        .ok_or("first location key")?;
    let second = second_child
        .instance_id()
        .value()
        .ok_or("second location key")?;
    assert_ne!(first, second, "each external route has its own exact key");
    assert_eq!(
        first_child.parent_location_id().value(),
        first_parent.instance_id().value(),
        "the first occurrence has its own authored parent"
    );
    let exact = starmap::read_location_instances(
        &input.archive,
        ContainerPlacementSelection::ExactOccurrence {
            root_entry_index: second.root_entry_index(),
            child_node_indices: second.child_node_indices(),
            external_child_step_indices: second.external_child_step_indices(),
        },
        StarmapTraversalBudget::new(),
        None,
    )?;
    let [row] = exact.location_instances.as_slice() else {
        return Err("one exact physical location".into());
    };
    assert_eq!(
        row.instance_id().value(),
        Some(second),
        "exact location key"
    );
    assert_eq!(
        row.parent_location_id().value(),
        second_parent.instance_id().value(),
        "an unselected parent is retained in the proof"
    );
    assert_eq!(
        row.world_position().value(),
        Some(&[10.0, 0.0, 0.0]),
        "the reused location uses the exact occurrence transform"
    );
    assert_companions(row)
}

fn assert_companions(row: &LocationInstance) -> TestResult {
    let declaration = row.declaration();
    assert_eq!(
        declaration.class().value().map(String::as_str),
        Some("FixtureMediumSettlement"),
        "future authored names remain independent exact text"
    );
    assert_eq!(
        declaration.class().origin().contains_encrypted_source(),
        Some(true),
        "class text inherits its physical descriptor's encryption"
    );
    let context = declaration.hierarchical_context().ok_or("context lookup")?;
    let context = context.document().value().ok_or("typed context document")?;
    assert_eq!(
        context.version().value().map(String::as_str),
        Some("2"),
        "context companion remains available through typed data"
    );
    let entity = declaration.ent_xml().ok_or("entity lookup")?;
    assert!(
        entity.inventory_complete() && entity.selected().value().is_some(),
        "same-container entity identity is uniquely proved"
    );
    Ok(())
}

#[test]
fn a_non_root_entry_is_outside_the_physical_selection() -> TestResult {
    let input = source()?;
    let selected: LocationPlacementSnapshot = starmap::read_location_instances(
        &input.archive,
        ContainerPlacementSelection::SystemRoot(entry_index(&input.archive, TILE)?),
        StarmapTraversalBudget::new(),
        None,
    )?;
    assert_eq!(
        selected.coverage.root,
        ContainerRootStatus::Absent,
        "an ordinary reused tile cannot own a system selection"
    );
    assert_eq!(
        selected.coverage.attempted_descriptors, 0,
        "non-root selection ends at directory admission"
    );
    assert!(
        selected.location_instances.is_empty(),
        "the unattached tile produced no occurrence rows"
    );
    Ok(())
}

fn source() -> TestResult<Fixture> {
    let root = format!(
        r#"<ObjectContainer><ChildObjectContainers>
            <Child locationType="1" class="FixtureRoot" external="1" name="{TILE}" pos="2,0,0"/>
            <Child locationType="1" class="FixtureRoot" external="1" name="{TILE}" pos="7,0,0"/>
        </ChildObjectContainers></ObjectContainer>"#,
    );
    let tile = format!(
        r#"<ObjectContainer><ChildObjectContainers>
            <Child locationType="3" class="FixtureMediumSettlement" classGuid="{PRESET}" guid="{GUID}"
                pos="3,0,0" hasHierarchicalContextData="1"/>
        </ChildObjectContainers></ObjectContainer>"#,
    );
    let context = format!(r#"<HierarchicalContextData __version="2" preset="{PRESET}"/>"#);
    let entity = format!(r#"<Entity EntityCryGUID="{GUID}"/>"#);
    let tile = TestContainer::new()
        .entry(TestEntry::stored("tile.xml", tile.as_bytes()))
        .entry(TestEntry::stored(
            format!("hcd/{GUID}.xml"),
            context.as_bytes(),
        ))
        .entry(TestEntry::stored(
            "entdata/entity.entxml",
            entity.as_bytes(),
        ))
        .build();
    Fixture::open(
        &fixture::empty_database(),
        Input::Plain,
        vec![
            super::packed(ROOT, "alphasystem.xml", root.as_bytes()),
            TestEntry::stored(TILE, tile).encrypted(),
            TestEntry::stored(UNRELATED, b"not a readable container".as_slice()),
        ],
    )
}
