// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::{
    ShipBaseFacts, ShipSourceSnapshot, with_ship_sources_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, FieldOrigin, SourceInventory};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::super::super::SHIP_BASE_FACTS;
use super::super::{Route, TestResult};

#[path = "../it/catalog_ship_cargo_fixture.rs"]
mod fixture;

const SHIP_RECORD: u32 = 3;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("ship-cargo-fixture.p4k");
    let database = fixture::database(125, 975, false).build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &entry, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    with_ship_sources_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(SHIP_RECORD),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| compare(&snapshot, &catalog),
    )?
}

fn compare(snapshot: &ShipSourceSnapshot<'_>, catalog: &Value) -> TestResult<Route> {
    let [ship] = snapshot.rows() else {
        return Err("one exact Ship Record from the existing scoped SDK route".into());
    };
    let facts = ship.base_facts();
    let expected = catalog
        .pointer(SHIP_BASE_FACTS)
        .ok_or("canonical Ship base facts table")?;
    let observed = json!({"availability": snapshot.availability(), "rows": [facts]});
    assert_eq!(
        &observed, expected,
        "the scoped typed row has the canonical wire values, order and original source uses"
    );
    let uses = catalog
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("canonical original input uses")?;
    verify_origins(facts, snapshot.source_inventory(), uses)?;
    Route::queried_value(
        SHIP_BASE_FACTS,
        facts,
        "synthetic and installed selected Ship parity witnessed; full installed producer parity open",
    )
}

fn verify_origins(
    facts: &ShipBaseFacts,
    inventory: &SourceInventory,
    expected: &[Value],
) -> TestResult<()> {
    let cargo = facts.cargo();
    let [left, right] = cargo.mounts() else {
        return Err("both exact reached CargoGrid occurrences".into());
    };
    assert_eq!(
        facts.ship_record_index().value(),
        Some(&SHIP_RECORD),
        "the row belongs to the selected physical source Ship"
    );
    assert_eq!(
        cargo.scu().value(),
        Some(&2.5),
        "the selected physical inventory contributes once per reached mount"
    );
    assert_eq!(
        left.attachment().target_record_index().value(),
        Some(&fixture::GRID_RECORD),
        "the attachment retains its physical grid target"
    );
    for origin in [
        facts.ship_record_index().origin(),
        cargo.scu().origin(),
        left.position().origin(),
        left.capacity_scu().origin(),
        right.position().origin(),
        right.capacity_scu().origin(),
    ] {
        verify_use(origin, inventory, expected)?;
    }
    Ok(())
}

fn verify_use(
    origin: &FieldOrigin,
    inventory: &SourceInventory,
    expected: &[Value],
) -> TestResult<()> {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "each proven cargo operand keeps its encrypted DataForge marker"
    );
    assert!(
        !origin.source_uses_refs().is_empty(),
        "the canonical cargo field has an observed input"
    );
    for reference in origin.source_uses_refs() {
        let actual = inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("the SDK inventory retains the exact original field use")?;
        let canonical = expected
            .iter()
            .find(|used| used.get("id").and_then(Value::as_str) == Some(reference.as_str()))
            .ok_or("the canonical inventory retains the same original field use")?;
        assert_eq!(
            &serde_json::to_value(actual)?,
            canonical,
            "the physical source, field coordinate and role agree for {}",
            reference.as_str()
        );
    }
    Ok(())
}
