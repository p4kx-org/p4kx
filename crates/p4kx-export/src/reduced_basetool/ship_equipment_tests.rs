// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, TestEntity, TestEntry, TestInstance, TestLoadout, TestLoadoutEntry, TestPort,
    TestValue, cargo_grid_facility_database, test_key,
};
use serde_json::Value;

use super::ship_cargo::ShipCargo;
use super::ship_equipment;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const STANDARD: u128 = 0x1063_0401;
const STORAGE: u128 = 0x1063_0402;
const MINING: u128 = 0x1063_0414;
const SPARE: u128 = 0x1063_0415;
const MIXED: u128 = 0x1063_0416;
const MIXED_GUID: &str = "00000000-0000-0000-0000-000010630416";
const PORTS: [(&str, u128); 3] = [
    ("grid_1", STANDARD),
    ("grid_2", STANDARD),
    ("mining_grid", MINING),
];

#[test]
fn mixed_ship_keeps_physical_mounts_and_unmounted_definitions_and_separates_freight() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let cargo = parsed.with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
        snapshot
            .rows()
            .iter()
            .filter_map(|ship| {
                let facts = ship.base_facts();
                facts
                    .ship_record_index()
                    .value()
                    .copied()
                    .map(|index| (index, ShipCargo::from(facts)))
            })
            .collect()
    })?;
    let staged = ship_equipment::collect(&parsed, scratch.path(), None, cargo)?;
    assert_eq!(staged.loadouts.len(), 2, "only admitted physical Ships");
    assert_eq!(
        staged.cargo_grids.len(),
        3,
        "the definition survey includes an unmounted grid"
    );
    assert_eq!(
        staged.cargo_placements.len(),
        5,
        "each resolved physical occurrence remains"
    );
    verify_ship(&serde_json::to_value(&staged.ship_base_facts)?)?;
    verify_loadout(&serde_json::to_value(&staged.loadouts)?)?;
    Ok(())
}

fn verify_ship(facts: &Value) -> TestResult {
    let rows = facts
        .pointer("/rows")
        .and_then(Value::as_array)
        .ok_or("ShipBaseFacts rows")?;
    let mixed = rows
        .iter()
        .find(|row| row.pointer("/shipGuid/value").and_then(Value::as_str) == Some(MIXED_GUID))
        .ok_or("mixed ship base facts")?;
    assert_eq!(
        mixed.pointer("/cargo/scu/value").and_then(Value::as_f64),
        Some(6.0)
    );
    assert_eq!(
        mixed.pointer("/cargo/freight/scu").and_then(Value::as_f64),
        Some(4.0)
    );
    assert_eq!(
        mixed
            .pointer("/cargo/freight/complete")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        mixed
            .pointer("/cargo/freight/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true)
    );
    for (index, port) in [0, 1, 2].into_iter().enumerate() {
        let path = format!("/cargo/mounts/{index}/position/value/portOrdinals/0");
        assert_eq!(mixed.pointer(&path).and_then(Value::as_u64), Some(port));
    }
    Ok(())
}

fn verify_loadout(loadouts: &Value) -> TestResult {
    let rows = loadouts.as_array().ok_or("Loadout rows")?;
    let mixed = rows
        .iter()
        .find(|row| row.pointer("/entityGuid/value").and_then(Value::as_str) == Some(MIXED_GUID))
        .ok_or("mixed ship loadout")?;
    assert_eq!(
        mixed.pointer("/entityKind/value").and_then(Value::as_str),
        Some("ship")
    );
    assert_eq!(
        mixed
            .pointer("/ports/2/itemClassName/value")
            .and_then(Value::as_str),
        Some("FixtureMiningGrid")
    );
    assert!(
        mixed.pointer("/ports/2/sourceConstraints").is_none(),
        "the reduced port omits source constraints"
    );
    Ok(())
}

fn database() -> Vec<u8> {
    let mounts = PORTS
        .map(|(port, guid)| TestLoadoutEntry::reference(port, guid))
        .to_vec();
    let mixed_ship = PORTS.into_iter().fold(
        TestEntity::new(
            MIXED,
            "FixtureMixedCargoHost",
            "libs/foundry/records/entities/spaceships/fixture_mixed_cargo_host.xml",
        )
        .vehicle("", "")
        .loadout(TestLoadout::Manual(mounts)),
        |ship, (port, _)| ship.port(TestPort::new(port)),
    );
    cargo_grid_facility_database()
        .record(grid(MINING, "FixtureMiningGrid", true).record())
        .record(grid(SPARE, "FixtureUninstalledGrid", false).record())
        .record(mixed_ship.record())
        .build()
}

fn grid(guid: u128, class: &str, mining: bool) -> TestEntity {
    TestEntity::new(
        guid,
        class,
        format!("libs/foundry/records/entities/scitem/fixture/{class}.xml"),
    )
    .attach_type("CargoGrid")
    .container(STORAGE)
    .component(TestValue::Struct(
        TestInstance::new("SCItemCargoGridParams")
            .value("miningOnly", TestValue::Bool(mining))
            .value("invisible", TestValue::Bool(false)),
    ))
}
