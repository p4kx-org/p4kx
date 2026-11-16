// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::ptr;
use std::sync::Arc;

use p4kx_extraction::{CancelToken, Guid, SourceInventory};
use p4kx_testkit::{TestContainer, TestElement, TestEntry};
use serde::Serialize;
use serde_json::{Value, json};

use super::{Limits, external_at, guid, open, read, root, system};
use crate::catalog_read::original::with_entry_capture;
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::catalog_table::CatalogTable;
use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_containers::{PlacementScope, Placements};

const REPEATED_TILES: usize = 8;

#[test]
fn repeated_container_occurrences_share_only_source_owned_data() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let system_xml = root(vec![
        external_at("tile", "10,0,0"),
        external_at("tile", "20,0,0"),
    ]);
    let record = guid(1);
    let tile = root(vec![
        TestElement::new("Child")
            .attribute("pos", "1,0,0")
            .attribute("starMapRecord", record.to_string())
            .attribute("locationType", "2")
            .attribute("label", "Storage canary"),
    ]);
    let mut placements = read(
        dir.path(),
        system(&system_xml, &[("tile", tile)]),
        Limits::default(),
    );
    verify_lookup_release(&mut placements, record);
    let containers = placements.take_instances();
    let [_, first, second] = containers.rows.as_slice() else {
        return Err("expected the system and two physical occurrences of one tile".into());
    };
    let first_wire = serde_json::to_value(first)?;
    let texts = first
        .text_cells_for_test()
        .into_iter()
        .zip(second.text_cells_for_test());
    for (field, (left, right)) in ["system", "sourcePath", "declaredPath"]
        .into_iter()
        .zip(texts)
    {
        let left_text = left.value().ok_or("present occurrence text")?;
        let right_text = right.value().ok_or("present repeated occurrence text")?;
        assert!(
            ptr::eq(left_text.as_str(), right_text.as_str()),
            "shared bytes for {field}"
        );
        let original_cell = left.map_ref(|text| text.as_str().to_owned());
        assert_eq!(
            first_wire.get(field),
            Some(&serde_json::to_value(original_cell)?),
            "original wire text, state and origin for {field}"
        );
    }
    let [system_name, _, first_path] = first.text_cells_for_test();
    let [repeated_system_name, _, repeated_path] = second.text_cells_for_test();
    assert!(
        ptr::eq(
            system_name.origin().source_uses_refs(),
            repeated_system_name.origin().source_uses_refs()
        ),
        "one sourced system cell is reused"
    );
    assert_ne!(
        first_path.origin(),
        repeated_path.origin(),
        "identical reference text retains its distinct authored slots"
    );
    verify_occurring_locations(&placements.take_locations().rows)
}

fn verify_lookup_release(placements: &mut Placements, record: Guid) {
    assert!(
        placements.ambiguity_proof(record).is_some(),
        "both actual record placements were indexed"
    );
    placements.release_record_lookup();
    assert!(
        placements.ambiguity_proof(record).is_none(),
        "lookup data is released before writing"
    );
}

fn verify_occurring_locations(
    rows: &[crate::starmap_containers::location_instance::LocationInstance],
) -> Result<(), Box<dyn Error>> {
    let [first, second] = rows else {
        return Err("expected one typed location per physical tile occurrence".into());
    };
    let declaration = first.declaration_for_test();
    assert!(
        Arc::ptr_eq(declaration, second.declaration_for_test()),
        "one immutable declaration is shared across two routes"
    );
    let original = serde_json::to_value(declaration.as_ref())?;
    let first = serde_json::to_value(first)?;
    let second = serde_json::to_value(second)?;
    for row in [&first, &second] {
        assert_eq!(
            row.get("declaration"),
            Some(&original),
            "original wire declaration"
        );
    }
    assert_ne!(
        first.pointer("/instanceId/value"),
        second.pointer("/instanceId/value"),
        "both routes retain physical identities"
    );
    assert_eq!(
        first.pointer("/worldPosition/value"),
        Some(&json!([11.0, 0.0, 0.0])),
        "first composed placement"
    );
    assert_eq!(
        second.pointer("/worldPosition/value"),
        Some(&json!([21.0, 0.0, 0.0])),
        "second composed placement"
    );
    Ok(())
}

fn spool_entries() -> Vec<TestEntry> {
    let system_xml = root(
        (1..=REPEATED_TILES)
            .map(|at| external_at("tile", &format!("{at},0,0")))
            .collect(),
    );
    let tile = root(vec![
        TestElement::new("Child")
            .attribute("pos", "1,0,0")
            .attribute("starMapRecord", guid(2).to_string())
            .attribute("locationType", "2"),
    ]);
    let mut entries = system(&system_xml, &[("tile", tile)]);
    entries.push(TestEntry::stored(
        "Data\\ObjectContainers\\PU\\system\\a\\asystem.socpak",
        TestContainer::new()
            .entry(TestEntry::stored(
                "asystem.xml",
                root(Vec::new()).as_bytes(),
            ))
            .build(),
    ));
    entries
}

#[test]
fn catalog_spool_replays_the_original_order_and_every_row_origin() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let scratch = tempfile::tempdir_in(dir.path())?;
    let archive = open(dir.path(), spool_entries());
    let (mut spooled, mut typed, inventory) = with_entry_capture(&archive, |capture| {
        let scope = PlacementScope::RecordSelection;
        let spooled = Placements::read_for_catalog(
            &archive,
            None,
            capture,
            scope,
            &JumpEndpointRecords::default(),
            scratch.path(),
        )?;
        let inventory = capture.source_uses().freeze()?;
        let typed = Placements::read(&archive, None, capture, scope)?;
        Ok((spooled, typed, inventory))
    })?;

    let (availability, spools) = spooled.take_spools();
    assert_eq!(
        spooled.take_instances().rows.len(),
        0,
        "no retained typed container rows"
    );
    assert_eq!(
        spooled.take_locations().rows.len(),
        0,
        "no retained typed location rows"
    );
    let containers = StagedCatalogTable::new(availability, spools.containers, None);
    let cancel = CancelToken::new();
    let locations = StagedCatalogTable::new(availability, spools.locations, Some(&cancel));
    assert_eq!(
        containers.len(),
        REPEATED_TILES + 2,
        "both roots and every tile"
    );
    assert_eq!(
        locations.len(),
        REPEATED_TILES,
        "one source location per tile"
    );
    let container_rows = verify_spooled_table(&containers, &typed.take_instances(), &inventory)?;
    let location_rows = verify_spooled_table(&locations, &typed.take_locations(), &inventory)?;
    verify_placement_links(&container_rows, &location_rows)?;
    verify_location_cancel(&locations, &cancel);
    drop((containers, locations));
    assert_eq!(
        fs::read_dir(scratch.path())?.count(),
        0,
        "scratch is removed"
    );
    Ok(())
}

fn verify_spooled_table(
    staged: &StagedCatalogTable,
    typed: &CatalogTable<impl Serialize>,
    inventory: &SourceInventory,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let expected = serde_json::to_vec(typed)?;
    for pass in 1..=2 {
        assert_eq!(
            serde_json::to_vec(staged)?,
            expected,
            "identical availability, row order and origins on pass {pass}"
        );
    }
    let mut table: Value = serde_json::from_slice(&expected)?;
    assert_eq!(
        table["availability"], "available",
        "spooled and typed roots are available"
    );
    let known: HashSet<_> = inventory
        .uses()
        .iter()
        .map(|usage| usage.id().as_str())
        .collect();
    let Value::Array(rows) = table["rows"].take() else {
        return Err("expected staged rows".into());
    };
    for row in &rows {
        verify_spooled_origins(row, &known)?;
    }
    Ok(rows)
}

fn verify_spooled_origins(row: &Value, known: &HashSet<&str>) -> Result<(), Box<dyn Error>> {
    match row {
        Value::Object(fields) => {
            for (name, value) in fields {
                if name == "sourceUsesRefs" {
                    for reference in value.as_array().ok_or("expected source-use references")? {
                        let id = reference.as_str().ok_or("expected a source-use ID")?;
                        assert!(known.contains(id), "source use in the production inventory");
                    }
                } else {
                    verify_spooled_origins(value, known)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                verify_spooled_origins(value, known)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn verify_placement_links(containers: &[Value], locations: &[Value]) -> Result<(), Box<dyn Error>> {
    assert_eq!(
        containers[0]["system"]["value"], "a",
        "systems use folder order"
    );
    assert_eq!(containers[1]["system"]["value"], "t", "second system root");
    for (container, location) in containers[2..].iter().zip(locations) {
        let id = container
            .pointer("/instanceId/value")
            .ok_or("expected a container ID")?;
        assert_eq!(
            location.pointer("/containerInstanceId/value"),
            Some(id),
            "each location links to its original physical occurrence"
        );
    }
    Ok(())
}

fn verify_location_cancel(locations: &StagedCatalogTable, cancel: &CancelToken) {
    cancel.cancel();
    assert!(
        serde_json::to_vec(locations).is_err(),
        "cancelled location replay"
    );
    assert!(
        matches!(
            locations.take_failure().map(crate::CatalogError::from),
            Some(crate::CatalogError::Cancelled)
        ),
        "location cancellation is recovered as the typed catalog error"
    );
}
