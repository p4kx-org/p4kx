// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::crafting::CrafterHostSnapshot;
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout,
    TestLoadoutEntry, TestStruct, TestValue, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::{Route, TestResult};

pub(in super::super) const DATASET: &str = "/projections/crafterHosts";

const HOST: u32 = 0;
const MOUNTED: u32 = 1;
const OTHER_HOST: u32 = 3;
const EMPTY_SHIP: u32 = 4;
const BROKEN_SHIP: u32 = 5;
const ALIAS_GUID: u128 = 0x77;
const COMPONENT: &str = "CrafterComponentParams";

pub(super) fn read() -> TestResult<Route> {
    let (_directory, archive) = open()?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog_from(&archive, &entry, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let table = catalog
        .pointer(DATASET)
        .ok_or("the canonical crafter host table")?;
    let rows = table["rows"].as_array().ok_or("host rows")?;
    assert_eq!(
        rows.len(),
        2,
        "the Catalog has two different physical ship hosts"
    );
    assert!(
        rows.iter()
            .any(|row| row.pointer("/shipRecordIndex/value") == Some(&json!(OTHER_HOST))),
        "another host exists outside the selected scope"
    );
    let expected: Vec<_> = rows
        .iter()
        .filter(|row| row.pointer("/shipRecordIndex/value") == Some(&json!(HOST)))
        .collect();

    let snapshot = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_crafter_hosts(HOST, None)?;
    verify_mounts(&snapshot)?;
    assert_eq!(
        &serde_json::to_value(snapshot.availability)?,
        &table["availability"],
        "original availability"
    );
    assert_eq!(
        serde_json::to_value(&snapshot.rows)?,
        json!(expected),
        "same selected physical host projection and field origins"
    );
    Route::queried_rows(
        DATASET,
        &snapshot.rows,
        "exact synthetic single-host producer parity available; installed producer parity open",
    )
}

#[test]
fn single_host_empty_scope_and_missing_scope_are_distinct() -> TestResult<()> {
    let (_directory, archive) = open()?;
    let entry = DataForgeEntry::read(&archive)?;
    let empty = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_crafter_hosts(EMPTY_SHIP, None)?;
    assert_eq!(
        empty.host_state,
        SourceValueState::Present,
        "the empty ship Loadout was resolved"
    );
    assert!(empty.rows.is_empty(), "no mount was authored");
    let not_ship = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_crafter_hosts(MOUNTED, None)?;
    assert_eq!(
        not_ship.host_state,
        SourceValueState::Absent,
        "the selected physical record is an Item"
    );
    assert!(
        not_ship.rows.is_empty(),
        "an Item does not admit a ship host"
    );
    let missing = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_crafter_hosts(u32::MAX, None)?;
    assert_eq!(
        missing.host_state,
        SourceValueState::Absent,
        "the selected Record does not exist"
    );
    assert!(missing.rows.is_empty(), "no unrelated ship is resolved");
    let broken = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_crafter_hosts(BROKEN_SHIP, None)?;
    assert_eq!(
        broken.host_state,
        SourceValueState::Unreadable,
        "an admitted but unresolvable ship is not absent"
    );
    assert!(
        broken.rows.is_empty(),
        "the malformed ship supplies no proved mounts"
    );
    Ok(())
}

fn verify_mounts(snapshot: &CrafterHostSnapshot) -> TestResult<()> {
    assert_eq!(
        snapshot.host_state,
        SourceValueState::Present,
        "the original ship has a resolved Loadout"
    );
    let [row] = snapshot.rows.as_slice() else {
        return Err("one physical host/crafter group".into());
    };
    assert_eq!(
        row.ship_record_index().value(),
        Some(&HOST),
        "scoped host identity"
    );
    assert_eq!(
        row.crafter().target_record_index().value(),
        Some(&MOUNTED),
        "the exact resolved station class selects only one GUID alias"
    );
    let [first, second] = row.mount_positions() else {
        return Err("two distinct authored mounts".into());
    };
    let first_path = first.value().ok_or("the first original port route")?;
    let second_path = second.value().ok_or("the second original port route")?;
    assert_eq!(
        first_path.loadout_record_index(),
        HOST,
        "the mount joins the exact Loadout root"
    );
    assert_ne!(
        first_path.port_ordinals(),
        second_path.port_ordinals(),
        "both repeated physical mounts survive"
    );
    assert_eq!(
        first_path.port_ordinals().len(),
        2,
        "the empty parent and its child are retained"
    );
    for mount in [first, second] {
        assert_eq!(
            mount.origin().contains_encrypted_source(),
            Some(true),
            "each mount retains its encrypted source"
        );
        assert!(
            mount.origin().source_uses_refs().iter().all(|reference| {
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|source| source.id() == reference)
            }),
            "mount source uses resolve in the one result-local inventory"
        );
    }
    Ok(())
}

fn open() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("synthetic-crafter-hosts.p4k");
    let nested = TestLoadout::Manual(vec![TestLoadoutEntry::empty("rack").loadout(
        TestLoadout::Manual(vec![
            TestLoadoutEntry::class("first_slot", "mount_station"),
            TestLoadoutEntry::class("second_slot", "mount_station"),
        ]),
    )]);
    let other = TestLoadout::Manual(vec![TestLoadoutEntry::class("slot", "other_station")]);
    let database = entity_schema(DataForgeBuilder::new().version(8))
        .structure(TestStruct::new(COMPONENT).parent("DataForgeComponentParams"))
        .record(ship(0x41, "source_host").loadout(nested).record())
        .record(station("mount_station").record())
        .record(station("other_station").record())
        .record(ship(0x42, "other_host").loadout(other).record())
        .record(ship(0x43, "empty_host").record())
        .record(
            ship(0x44, "broken_host")
                .record()
                .with_instance_index(u16::MAX),
        );
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    Ok((directory, Archive::open_with_key(path, Some(test_key()))?))
}

fn ship(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(
        guid,
        class,
        format!("libs/foundry/records/entities/spaceships/{class}.xml"),
    )
}

fn station(class: &str) -> TestEntity {
    TestEntity::new(
        ALIAS_GUID,
        class,
        format!("libs/foundry/records/entities/scitem/ships/{class}.xml"),
    )
    .component(TestValue::Struct(TestInstance::new(COMPONENT)))
}
