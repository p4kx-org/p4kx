// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::starmap::{
    DatasetAvailability, LocationPhysicsDescriptorSelection, LocationPhysicsDescriptorStatus,
    read_location_physics_descriptor,
};
use p4kx_extraction::SourceValueState;
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{ROOT, TILE, child, entry_index, normalized, packed, read_occurrence, read_root};
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

const GRID: &[u8] = br#"<ObjectContainer gridRadius="4.0">
<PhysicsGridPart shape="voxel">
  <PhysicsGridVoxel filename="Objects\Test\floor.cgf"/>
  <Unknown marker="preserve">test</Unknown>
</PhysicsGridPart>
<PhysicsGrid gravity="0, 2, -8.0" hasPart="1"><PhysicsGridChildParts/></PhysicsGrid>
<CargoGrid entitySuperGUID="synthetic"/>
</ObjectContainer>"#;

#[test]
fn shared_grid_matches_catalog_and_both_exact_sdk_routes() -> TestResult {
    let fixture = with_grid(GRID)?;
    let root = entry_index(&fixture.archive, ROOT)?;
    let (_, catalog) = fixture.diagnostic()?;
    let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        schema.is_valid(&catalog),
        "the Catalog schema accepts a populated grid with its gravity vector"
    );
    let rows = proof::rows(&catalog, "/placements/locationPhysicsGrids/rows")?;
    let [canonical] = rows else {
        return Err("one physical declaration despite both placements".into());
    };
    let selected = read_root(&fixture.archive, root)?;
    assert!(
        matches!(
            selected.location_physics_availability,
            DatasetAvailability::Available
        ),
        "complete positive grid scope"
    );
    let [grid] = selected.location_physics_grids.as_slice() else {
        return Err("one SDK physical declaration".into());
    };
    let joined = selected
        .container_instances
        .iter()
        .filter(|row| row.container.value() == grid.container.value())
        .collect::<Vec<_>>();
    let [left, right] = joined.as_slice() else {
        return Err("two original occurrences join to the same descriptor".into());
    };
    let sdk = json!({"provenance": selected.source_inventory});
    let sdk_row = serde_json::to_value(grid.as_ref())?;
    assert_eq!(
        normalized(&sdk_row, &sdk)?,
        normalized(canonical, &catalog)?,
        "both producers retain the same fields and physical provenance"
    );
    assert_eq!(
        canonical.pointer("/declarations/1/fields/target/value/type"),
        Some(&json!("matched")),
        "the voxel target remains a directory reference"
    );
    assert_eq!(
        canonical.pointer("/declarations/2/fields/controls/gravity/value"),
        Some(&json!([0.0, 2.0, -8.0])),
        "the original gravity vector is exported with its components in order"
    );
    for row in [left, right] {
        let key = row.instance_id.value().ok_or("complete occurrence")?;
        let queried = read_occurrence(&fixture.archive, key)?;
        check_exact(&queried, key, canonical, &catalog)?;
    }
    Ok(())
}

fn check_exact(
    queried: &super::ContainerPlacementSnapshot,
    key: &super::ContainerInstanceKey,
    canonical: &Value,
    catalog: &Value,
) -> TestResult {
    let [requested] = queried.container_instances.as_slice() else {
        return Err("one exact occurrence".into());
    };
    assert_eq!(
        requested.instance_id.value(),
        Some(key),
        "exact route proof survives"
    );
    let [definition] = queried.location_physics_grids.as_slice() else {
        return Err("one physical descriptor for the exact route".into());
    };
    let snapshot = json!({"provenance": queried.source_inventory});
    assert_eq!(
        normalized(&serde_json::to_value(definition.as_ref())?, &snapshot)?,
        normalized(canonical, catalog)?,
        "exact selection retains the canonical grid data and complete provenance"
    );
    Ok(())
}

#[test]
fn descriptor_selection_proves_absence_only_here_and_rejects_an_inner_alias() -> TestResult {
    let fixture = with_grid(
        br#"<ObjectContainer><CargoGrid entitySuperGUID="synthetic"/></ObjectContainer>"#,
    )?;
    let container = entry_index(&fixture.archive, TILE)?;
    let actual = read_location_physics_descriptor(
        &fixture.archive,
        LocationPhysicsDescriptorSelection::new(container, 0),
        None,
    )?;
    assert_eq!(
        actual.status,
        LocationPhysicsDescriptorStatus::Read,
        "selected physical descriptor parsed"
    );
    assert_eq!(
        actual.present.value(),
        Some(&false),
        "complete descriptor has no location physics"
    );
    assert!(
        actual.grid.is_none(),
        "CargoGrid does not supply any location physics"
    );
    let alias = read_location_physics_descriptor(
        &fixture.archive,
        LocationPhysicsDescriptorSelection::new(container, 1),
        None,
    )?;
    assert_eq!(
        alias.status,
        LocationPhysicsDescriptorStatus::SelectorMismatch,
        "a sibling XML cannot alias the canonical descriptor"
    );
    assert_eq!(
        alias.present.state(),
        SourceValueState::Unreadable,
        "selector mismatch is not proof of absence"
    );
    Ok(())
}

fn with_grid(body: &[u8]) -> TestResult<Fixture> {
    let root = TestElement::new("ObjectContainer")
        .child(
            TestElement::new("ChildObjectContainers")
                .child(child(TILE, "2,0,0"))
                .child(child(TILE, "7,0,0")),
        )
        .to_xml();
    let tile = TestContainer::new()
        .entry(TestEntry::stored("tile.xml", body))
        .entry(TestEntry::stored("sibling.xml", GRID))
        .build();
    let entries = vec![
        packed(ROOT, "alphasystem.xml", root.as_bytes()),
        TestEntry::stored(TILE, tile).encrypted(),
        TestEntry::stored("Data/Objects/Test/floor.cgf", [b'X'; 8]),
    ];
    Fixture::open(&fixture::empty_database(), Input::Plain, entries)
}
