// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_testkit::TestElement;
use serde_json::{Value, json};

use super::super::{Limits, external_at, open, root, system};
use crate::StarmapTraversalBudget;
use crate::catalog_queries::starmap::{
    ContainerInstance, ContainerPlacementSelection, read_container_placements,
};
use crate::catalog_read::original::with_entry_capture;
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_containers::{PlacementScope, Placements};

#[test]
fn same_document_local_ordinals_keep_distinct_inline_and_external_routes()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let scratch = tempfile::tempdir_in(directory.path())?;
    let branching = external_at("branch", "10,0,0").child(
        TestElement::new("ChildObjectContainers").child(external_at("inline-leaf", "2,0,0")),
    );
    let branch = root(vec![
        TestElement::new("Ignored"),
        TestElement::new("Ignored"),
        external_at("external-leaf", "7,0,0"),
    ]);
    let entries = system(
        &root(vec![branching]),
        &[
            ("branch", branch),
            ("external-leaf", root(Vec::new())),
            ("inline-leaf", root(Vec::new())),
        ],
    );
    let archive = open(directory.path(), entries);
    let mut placements = with_entry_capture(&archive, |capture| {
        Placements::read_for_catalog(
            &archive,
            None,
            Limits::default(),
            capture,
            PlacementScope::ArchiveInventory,
            &JumpEndpointRecords::default(),
            scratch.path(),
        )
    })?;
    let (availability, spools) = placements.take_spools();
    let table = serde_json::to_value(StagedCatalogTable::new(
        availability,
        spools.containers,
        None,
    ))?;
    check_routes(&archive, &table)
}

#[test]
fn exact_route_records_each_external_document_transition() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let inline = TestElement::new("Child")
        .attribute("pos", "2,0,0")
        .child(TestElement::new("ChildObjectContainers").child(external_at("second", "3,0,0")));
    let entries = system(
        &root(vec![external_at("first", "1,0,0")]),
        &[
            ("first", root(vec![inline])),
            ("second", root(vec![external_at("leaf", "4,0,0")])),
            ("leaf", root(Vec::new())),
        ],
    );
    let archive = open(directory.path(), entries);
    let root = archive
        .entries()
        .iter()
        .find(|entry| entry.path().rsplit(['/', '\\']).next() == Some("tsystem.socpak"))
        .ok_or("synthetic system root Entry")?;
    let all = read_container_placements(
        &archive,
        ContainerPlacementSelection::SystemRoot(u32::try_from(root.index())?),
        StarmapTraversalBudget::new(),
        None,
    )?;
    let leaf = all.container_instances.last().ok_or("two-hop leaf")?;
    let key = leaf.instance_id.value().ok_or("two-hop occurrence route")?;
    assert_eq!(key.child_node_indices().len(), 4, "four authored Children");
    assert_eq!(
        key.external_child_step_indices(),
        &[1, 3],
        "each first Child in an entered external document is at its original step",
    );
    assert_eq!(
        leaf.world_position.value(),
        Some(&[10.0, 0.0, 0.0]),
        "the pose composes all four authored steps",
    );
    check_exact_sdk_route(&archive, leaf, &serde_json::to_value(leaf)?)
}

fn check_routes(archive: &Archive, table: &Value) -> Result<(), Box<dyn Error>> {
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("spooled rows")?;
    let [root, branch, external, inline] = rows.as_slice() else {
        return Err("expected one opening and two leaf occurrences below the root".into());
    };
    let external_key = route(external)?;
    let inline_key = route(inline)?;
    check_distinct_keys(external_key, inline_key);
    assert_eq!(
        external.pointer("/parentInstanceId/value"),
        Some(route(branch)?),
        "external route joins the opened external root",
    );
    assert_eq!(
        inline.pointer("/parentInstanceId/value"),
        Some(route(root)?),
        "inline route joins its original containing root",
    );
    for field in [
        "/container/value",
        "/worldPosition/value",
        "/instanceId/sourceUsesRefs",
    ] {
        let left = external
            .pointer(field)
            .ok_or("external occurrence contributor")?;
        let right = inline
            .pointer(field)
            .ok_or("inline occurrence contributor")?;
        assert_ne!(left, right, "distinct physical occurrence inputs: {field}");
    }
    check_sdk_routes(archive, external, inline)
}

fn check_sdk_routes(
    archive: &Archive,
    external: &Value,
    inline: &Value,
) -> Result<(), Box<dyn Error>> {
    use serde::Deserialize;

    let key = route(external)?;
    let root_entry_index = key
        .get("rootEntryIndex")
        .ok_or("spooled root Entry index")?;
    let child_node_indices = key
        .get("childNodeIndices")
        .ok_or("spooled Child ordinals")?;
    let root = u32::deserialize(root_entry_index)?;
    let ordinals = Vec::<u32>::deserialize(child_node_indices)?;
    let coarse = read_container_placements(
        archive,
        ContainerPlacementSelection::Occurrence {
            root_entry_index: root,
            child_node_indices: &ordinals,
        },
        StarmapTraversalBudget::new(),
        None,
    )?;
    assert!(
        coarse.coverage.is_complete(),
        "the coarse query checked the whole graph"
    );
    assert!(
        coarse.unresolved_container_links.is_empty(),
        "both children resolve"
    );
    assert!(
        coarse.external_child_step_indices.is_none(),
        "coarse selector omits the external steps"
    );
    let [external_row, inline_row] = coarse.container_instances.as_slice() else {
        return Err("the coarse selector returns both physical routes".into());
    };
    for (row, expected) in [(external_row, external), (inline_row, inline)] {
        check_sdk_row(row, expected)?;
        check_exact_sdk_route(archive, row, expected)?;
    }
    Ok(())
}

fn check_exact_sdk_route(
    archive: &Archive,
    row: &ContainerInstance,
    expected: &Value,
) -> Result<(), Box<dyn Error>> {
    let key = row.instance_id.value().ok_or("SDK physical route")?;
    let exact = read_container_placements(
        archive,
        ContainerPlacementSelection::ExactOccurrence {
            root_entry_index: key.root_entry_index(),
            child_node_indices: key.child_node_indices(),
            external_child_step_indices: key.external_child_step_indices(),
        },
        StarmapTraversalBudget::new(),
        None,
    )?;
    assert!(
        exact.coverage.is_complete(),
        "the exact query checked the whole graph"
    );
    assert!(
        exact.unresolved_container_links.is_empty(),
        "the exact child resolves"
    );
    assert_eq!(
        exact.external_child_step_indices.as_deref(),
        Some(key.external_child_step_indices()),
        "the exact selector exposes the requested document route",
    );
    let [only] = exact.container_instances.as_slice() else {
        return Err("the exact selector returns one physical route".into());
    };
    check_sdk_row(only, expected)
}

fn check_sdk_row(row: &ContainerInstance, expected: &Value) -> Result<(), Box<dyn Error>> {
    let actual = serde_json::to_value(row)?;
    for field in [
        "/instanceId/value",
        "/parentInstanceId/value",
        "/localPosition/value",
        "/worldPosition/value",
        "/worldRotation/value",
    ] {
        assert_eq!(
            actual.pointer(field),
            Some(
                expected
                    .pointer(field)
                    .ok_or("spooled route identity or pose")?
            ),
            "SDK row matches spooled field {field}",
        );
    }
    Ok(())
}

fn route(row: &Value) -> Result<&Value, Box<dyn Error>> {
    row.pointer("/instanceId/value")
        .ok_or_else(|| "original route".into())
}

fn check_distinct_keys(external: &Value, inline: &Value) {
    for field in ["rootEntryIndex", "childNodeIndices"] {
        assert!(
            external.get(field).is_some(),
            "the original route field {field} remains available"
        );
        assert_eq!(
            external.get(field),
            inline.get(field),
            "both declarations have the same original {field}",
        );
    }
    assert_eq!(
        external.get("externalChildStepIndices"),
        Some(&json!([1])),
        "the second Child comes from the external document",
    );
    assert_eq!(
        inline.get("externalChildStepIndices"),
        Some(&json!([])),
        "both Children stay within the root document",
    );
    assert_ne!(external, inline, "each physical route has its own wire key");
}
