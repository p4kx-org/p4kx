// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{self, WeaponSourceSnapshot};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope, SourceInventory, SourceLocation};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::super::super::AMMO_CONTAINERS;
use super::ammo_binding_witness;
use super::{Route, TestResult};

#[expect(
    dead_code,
    reason = "The parity witness uses the readable two-container fixture."
)]
#[path = "../it/catalog_ammo_containers_fixture.rs"]
mod fixture;

const OWNER: u32 = 2;
const CONTAINERS: &str = "/configurations/ammoContainers/rows";

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ammo-containers.p4k");
    let database = fixture::database(fixture::Case::Valid);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog = ammo_binding_witness::normal(&archive, &entry)?;
    let mut route = None;
    let summary = equipment::visit_weapon_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |index, snapshot| -> TestResult<()> {
            assert_eq!(
                index, OWNER,
                "the visitor keeps the physical container owner"
            );
            route = Some(verify(&snapshot, &catalog)?);
            Ok(())
        },
    )?;
    assert_eq!(
        summary.owner_count(),
        1,
        "the fixture admits exactly one container owner"
    );
    route.ok_or_else(|| "expected the canonical ammo container owner".into())
}

fn verify(snapshot: &WeaponSourceSnapshot<'_>, catalog: &Value) -> TestResult<Route> {
    let [owner] = snapshot.rows() else {
        return Err("the exact container-only fixture Item".into());
    };
    let [first, second] = owner.ammo_containers.as_slice() else {
        return Err("both physical container slots from the same Item".into());
    };
    assert!(
        owner.weapon_actions.is_empty(),
        "the Item authors only containers"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty()
            && snapshot.coverage().incomplete_fields.is_empty(),
        "both admitted component bodies and field scopes are readable"
    );
    for (row, ordinal, capacity) in [(first, 1, -7), (second, 2, 0)] {
        assert_eq!(
            row.owner().record_index().value(),
            Some(&OWNER),
            "physical Item"
        );
        assert_eq!(
            row.owner().component_ordinal().value(),
            Some(&ordinal),
            "original array slot, including the preceding non-container component"
        );
        assert_eq!(
            row.max_ammo_count().value(),
            Some(&capacity),
            "the typed quantity preserves the authored signed count"
        );
    }
    let rows = serde_json::to_value(&owner.ammo_containers)?;
    assert_eq!(
        Some(&rows),
        catalog.pointer(CONTAINERS),
        "the scoped query preserves the Normal Catalog's ordered rows and their field origins"
    );
    let [first, second] = rows.as_array().ok_or("the SDK row array")?.as_slice() else {
        return Err("the two serialized SDK rows".into());
    };
    verify_attachment_sources(first, second, snapshot.source_inventory(), catalog)?;
    Route::queried_rows(AMMO_CONTAINERS, &owner.ammo_containers, None)
}

fn verify_attachment_sources(
    first: &Value,
    second: &Value,
    inventory: &SourceInventory,
    catalog: &Value,
) -> TestResult<()> {
    assert_eq!(
        first.pointer("/attachableEntities/value"),
        Some(&json!([])),
        "the first slot retains its authored empty attachable array"
    );
    assert_eq!(
        second.pointer("/hideAttachments/value"),
        Some(&json!([])),
        "the second slot retains its authored empty hide array"
    );
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("the Normal Catalog's original contributor inventory")?;
    let [before, after] = array(first, "/hideAttachments/value")? else {
        return Err("the two ordered hide-attachment entries".into());
    };
    let threshold = |row: &Value, expected| {
        let cell = row
            .pointer("/value/ammoThreshold")
            .ok_or("the authored hide threshold")?;
        check_source(cell, &json!(expected), inventory, uses)
    };
    assert!(
        threshold(before, 7)? < threshold(after, 0)?,
        "the two hide thresholds retain distinct original value occurrences in source order"
    );
    let [attachment] = array(second, "/attachableEntities/value")? else {
        return Err("the sole authored attachable entity".into());
    };
    let port = attachment
        .pointer("/value/targetPort")
        .ok_or("the original attachable target port")?;
    check_source(
        port,
        &json!({"type":"gameText","sourceText":"fixture-target"}),
        inventory,
        uses,
    )?;
    Ok(())
}

fn array<'row>(row: &'row Value, pointer: &str) -> TestResult<&'row [Value]> {
    row.pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected the ordered array at {pointer}").into())
}

fn check_source(
    cell: &Value,
    value: &Value,
    inventory: &SourceInventory,
    catalog_uses: &[Value],
) -> TestResult<u64> {
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "authored field state"
    );
    assert_eq!(cell.get("value"), Some(value), "authored field value");
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the original field is encrypted"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "the original field source set is complete"
    );
    let references = cell
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("the original field contributor set")?;
    let (used, ordinal) = inventory
        .iter_uses()
        .find_map(|used| {
            let SourceLocation::Subresource {
                namespace,
                indices,
                field,
            } = used.location()
            else {
                return None;
            };
            let [record, ordinal] = indices.as_slice() else {
                return None;
            };
            (namespace == "dataforgeValue"
                && *record == u64::from(OWNER)
                && field == "$value"
                && used.scope() == InputScope::Payload
                && references
                    .iter()
                    .any(|id| id.as_str() == Some(used.id().as_str())))
            .then_some((used, *ordinal))
        })
        .ok_or("the SDK resolves an original value occurrence on the physical Item")?;
    let canonical = catalog_uses
        .iter()
        .find(|row| row.get("id").and_then(Value::as_str) == Some(used.id().as_str()))
        .ok_or("the Normal Catalog resolves the same original value occurrence")?;
    assert_eq!(
        &serde_json::to_value(used)?,
        canonical,
        "the physical original location, source and role agree after resolving the reference"
    );
    Ok(ordinal)
}

#[test]
fn ammo_container_slots_keep_ordered_fields_and_original_sources() -> TestResult<()> {
    read()?;
    Ok(())
}
