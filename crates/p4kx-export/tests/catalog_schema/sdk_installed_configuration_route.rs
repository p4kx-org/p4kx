// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::{
    FlightConfigurationKind, InstalledConfiguration, InstalledConfigurationSnapshot,
};
use p4kx_export::catalog_queries::{DatasetAvailability, ParsedCatalogQueries, equipment};
use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, SourceInventory, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout, TestRecord,
    TestValue, test_key,
};
use serde_json::Value;

use super::super::{Route, TestResult};
use super::{DECK_RECORD, DOCKED_RECORD, SEAT_RECORD, SHIP_RECORD, UNSELECTED_RECORD, fixture};

const DATASET: &str = "/placements/installedConfigurations";
const SEAT_ALIAS_RECORD: u32 = 5;
const BROKEN_SHIP_RECORD: u32 = 6;
const SHIP_ALIAS_RECORD: u32 = 7;
const EMPTY_SHIP_RECORD: u32 = 8;
const UNREADABLE_SHIP_RECORD: u32 = 9;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("installed-flight.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", flight_database().build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, options.with_full_provenance())?.json(),
    )?;
    check_competing_alias(&catalog)?;
    let snapshot = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_installed_configurations(SHIP_RECORD, None)?;
    compare(&snapshot, &catalog, SHIP_RECORD)?;
    check_occurrences(&snapshot)?;
    let docked = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_installed_configurations(DOCKED_RECORD, None)?;
    compare(&docked, &catalog, DOCKED_RECORD)?;
    assert_eq!(
        docked.rows.len(),
        2,
        "the docked vehicle has its own root and seat installation"
    );
    check_broken_sibling(&archive, &entry, &catalog)?;
    refuse_non_ship(&archive, &entry)?;
    let queries = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    check_fleet(&queries.parsed(None)?, &catalog)?;
    Route::queried_rows(
        DATASET,
        &snapshot.rows,
        "synthetic whole-fleet parity and selected installed Ship evidence are verified; installed fleet parity and cold memory remain open",
    )
}

fn flight_database() -> DataForgeBuilder {
    fixture::database()
        .record(
            TestEntity::new(
                fixture::SEAT,
                "FixtureSeat",
                "libs/foundry/records/entities/scitem/fixture/seat_variant.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(
                TestInstance::new("SCItemThrusterParams")
                    .value("thrustCapacity", TestValue::Single(99.0)),
            ))
            .record(),
        )
        .record(broken_ship())
        .record(
            ship(
                fixture::SHIP,
                "FixtureFacilityHostAlias",
                "libs/foundry/records/entities/spaceships/fixture_facility_host_alias.xml",
            )
            .component(TestValue::Struct(
                TestInstance::new("SCItemThrusterParams")
                    .value("thrustCapacity", TestValue::Single(5.0)),
            ))
            .record(),
        )
        .record(
            ship(
                fixture::SHIP + 8,
                "FixtureEmptyFlightHost",
                "libs/foundry/records/entities/spaceships/fixture_empty_flight_host.xml",
            )
            .record(),
        )
        .record(
            ship(
                fixture::SHIP + 9,
                "FixtureUnreadableFlightHost",
                "libs/foundry/records/entities/spaceships/fixture_unreadable_flight_host.xml",
            )
            .record()
            .with_instance_index(u16::MAX),
        )
}

fn ship(record: u128, name: &str, path: &str) -> TestEntity {
    TestEntity::new(record, name, path)
        .vehicle("", "")
        .loadout(TestLoadout::Manual(Vec::new()))
}

fn check_fleet(queries: &ParsedCatalogQueries<'_>, catalog: &Value) -> TestResult {
    let mut owners = Vec::new();
    let mut rows = Vec::new();
    let mut incomplete = 0;
    let summary = queries.visit_installed_configurations(None, |owner| -> TestResult {
        let index = owner.ship_record_index;
        owners.push(index);
        compare(owner, catalog, index)?;
        for row in &owner.rows {
            rows.push(serde_json::to_value(row)?);
        }
        incomplete += usize::from(!owner.is_complete());
        check_fleet_owner(owner)
    })?;
    assert_eq!(
        owners,
        [
            SHIP_RECORD,
            SHIP_ALIAS_RECORD,
            DOCKED_RECORD,
            BROKEN_SHIP_RECORD,
            EMPTY_SHIP_RECORD,
            UNREADABLE_SHIP_RECORD,
        ],
        "canonical GUID order retains aliases, empty owners and failed roots"
    );
    assert_eq!(
        summary.placement_count,
        rows.len(),
        "the terminal count covers every emitted placement"
    );
    assert_eq!(
        &Value::Array(rows),
        catalog
            .pointer("/placements/installedConfigurations/rows")
            .ok_or("canonical fleet rows")?,
        "all ordered rows and their per-value evidence match the producer"
    );
    assert_eq!(
        summary.owner_count,
        owners.len(),
        "even empty and unreadable roots count as visited owners"
    );
    assert_eq!(
        summary.incomplete_owner_count, incomplete,
        "the terminal count agrees with callback coverage"
    );
    assert!(
        matches!(summary.availability, DatasetAvailability::Available),
        "the authored thruster family anchors the fleet table"
    );
    Ok(())
}

fn check_fleet_owner(owner: &InstalledConfigurationSnapshot) -> TestResult {
    match owner.ship_record_index {
        SHIP_RECORD => {
            check_occurrences(owner)?;
            assert!(
                !owner.is_complete(),
                "the original dangling attachment remains visible"
            );
        }
        SHIP_ALIAS_RECORD => check_ship_alias(owner)?,
        BROKEN_SHIP_RECORD => assert!(
            !owner.is_complete(),
            "a readable flight row cannot hide its damaged sibling"
        ),
        EMPTY_SHIP_RECORD => {
            assert_eq!(
                owner.loadout_state,
                SourceValueState::Present,
                "a successfully resolved empty Loadout still exists"
            );
            assert!(
                owner.rows.is_empty() && owner.is_complete(),
                "empty resolved owner"
            );
        }
        UNREADABLE_SHIP_RECORD => {
            assert_eq!(
                owner.loadout_state,
                SourceValueState::Unreadable,
                "an admitted root with an invalid original body cannot resolve"
            );
            assert!(
                owner.rows.is_empty() && !owner.is_complete(),
                "failed physical root"
            );
            assert!(
                owner
                    .loadout_coverage
                    .incomplete_fields
                    .contains(&UNREADABLE_SHIP_RECORD),
                "the failure identifies the exact original owner"
            );
        }
        _ => {}
    }
    Ok(())
}

fn check_ship_alias(owner: &InstalledConfigurationSnapshot) -> TestResult {
    let [row] = owner.rows.as_slice() else {
        return Err("the physical Ship alias retains its own flight component".into());
    };
    assert_eq!(
        row.ship_record_index().value(),
        Some(&SHIP_ALIAS_RECORD),
        "a physical alias with the same GUID retains its own index"
    );
    assert!(
        row.position()
            .value()
            .ok_or("alias position")?
            .port_ordinals()
            .is_empty(),
        "the alias configuration belongs to its root"
    );
    Ok(())
}

#[test]
fn installed_configuration_visit_stops_on_cancellation_or_callback_failure() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("installed-flight-stop.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, None)?;
    let entry = DataForgeEntry::read(&archive)?;
    let cancel = CancelToken::new();
    let mut observed = Vec::new();
    let result =
        equipment::visit_installed_configurations_from(&archive, &entry, Some(&cancel), |owner| {
            observed.push(owner.ship_record_index);
            cancel.cancel();
            Ok(())
        });
    assert!(matches!(result, Err(CatalogError::Cancelled)));
    assert_eq!(
        observed,
        [SHIP_RECORD],
        "cancellation stops before any later callback"
    );

    let mut observed = Vec::new();
    let result = equipment::visit_installed_configurations(&archive, None, |owner| -> TestResult {
        observed.push(owner.ship_record_index);
        Err("consumer output failed".into())
    });
    assert_eq!(
        result
            .expect_err("the callback error reaches the caller")
            .to_string(),
        "consumer output failed"
    );
    assert_eq!(
        observed,
        [SHIP_RECORD],
        "callback failure stops before any later callback"
    );
    Ok(())
}

fn broken_ship() -> TestRecord {
    let broken = [u32::MAX.to_le_bytes(), 0_u32.to_le_bytes()].concat();
    TestEntity::new(
        fixture::SHIP + 7,
        "FixturePartialThruster",
        "libs/foundry/records/entities/spaceships/fixture_partial_thruster.xml",
    )
    .vehicle("", "")
    .loadout(TestLoadout::Manual(Vec::new()))
    .component(TestValue::Raw(broken))
    .component(TestValue::Struct(
        TestInstance::new("SCItemThrusterParams").value("thrustCapacity", TestValue::Single(8.0)),
    ))
    .record()
}

fn check_broken_sibling(archive: &Archive, entry: &DataForgeEntry, catalog: &Value) -> TestResult {
    let snapshot = full_proof::from_entry(archive, entry, full_proof::NO_LANGUAGES, None)?
        .read_installed_configurations(BROKEN_SHIP_RECORD, None)?;
    compare(&snapshot, catalog, BROKEN_SHIP_RECORD)?;
    let [root] = snapshot.rows.as_slice() else {
        return Err("the valid Thruster survives its damaged sibling".into());
    };
    let [configuration] = root.configurations() else {
        return Err("one valid Thruster is installed on the root".into());
    };
    assert_eq!(
        configuration.owner().component_ordinal().value(),
        Some(&3),
        "the valid Thruster retains the slot following the broken component"
    );
    let [owner] = snapshot.configuration_dependencies.as_slice() else {
        return Err("the physical root reports one flight configuration dependency".into());
    };
    assert_eq!(
        owner.thrusters.unreadable_bodies,
        [BROKEN_SHIP_RECORD],
        "the valid row coexists with the strict coverage failure for its original owner"
    );
    Ok(())
}

fn check_competing_alias(catalog: &Value) -> TestResult {
    let rows = catalog
        .pointer("/configurations/thrusters/rows")
        .and_then(Value::as_array)
        .ok_or("canonical Thruster declarations")?;
    assert!(
        rows.iter().any(|row| row
            .pointer("/ownerRecordIndex/value")
            .and_then(Value::as_u64)
            == Some(u64::from(SEAT_ALIAS_RECORD))),
        "the unselected alias has its own actual flight component",
    );
    Ok(())
}

fn compare(snapshot: &InstalledConfigurationSnapshot, catalog: &Value, root: u32) -> TestResult {
    let expected = catalog
        .pointer(DATASET)
        .ok_or("canonical installed configurations table")?;
    assert_eq!(
        expected.get("availability"),
        Some(&serde_json::to_value(snapshot.availability)?),
        "both outputs retain the canonical flight source-family availability",
    );
    let rows: Vec<_> = expected
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("canonical placements")?
        .iter()
        .filter(|row| {
            row.pointer("/shipRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(root))
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&snapshot.rows)?,
        serde_json::to_value(&rows)?,
        "the scoped query and producer agree on the physical Ship's values and field origins",
    );
    Ok(())
}

fn check_occurrences(snapshot: &InstalledConfigurationSnapshot) -> TestResult {
    assert_eq!(
        snapshot.loadout_state,
        SourceValueState::Present,
        "the original ship loadout is present"
    );
    assert_eq!(
        snapshot.rows.len(),
        3,
        "two direct seats and the nested deck seat each contribute"
    );
    let paths: [&[u32]; 3] = [&[0], &[1], &[2, 0]];
    for (row, expected) in snapshot.rows.iter().zip(paths) {
        check_occurrence(row, expected, &snapshot.source_inventory)?;
    }
    assert_eq!(
        snapshot
            .configuration_dependencies
            .iter()
            .map(|entry| entry.record_index)
            .collect::<Vec<_>>(),
        [SHIP_RECORD, SEAT_RECORD, DECK_RECORD],
        "docked and unresolved ports never consult flight configurations",
    );
    assert!(
        snapshot
            .loadout_coverage
            .incomplete_fields
            .contains(&SHIP_RECORD),
        "the omitted dangling Reference is visible in selected Loadout coverage"
    );
    Ok(())
}

fn check_occurrence(
    row: &InstalledConfiguration,
    expected: &[u32],
    inventory: &SourceInventory,
) -> TestResult {
    let position = row
        .position()
        .value()
        .ok_or("original installed position")?;
    assert_eq!(
        position.loadout_record_index(),
        SHIP_RECORD,
        "the scope uses a physical loadout owner"
    );
    assert_eq!(
        position.port_ordinals(),
        expected,
        "the original nested port ordinals survive"
    );
    let [link] = row.configurations() else {
        return Err("one configuration for each physical seat".into());
    };
    assert!(
        matches!(link.kind(), FlightConfigurationKind::Thruster),
        "the authored thruster family survives"
    );
    let owner = link.owner().record_index();
    assert_eq!(
        owner.value(),
        Some(&SEAT_RECORD),
        "every seat refers to the original physical Record"
    );
    assert_eq!(
        owner.origin().contains_encrypted_source(),
        Some(true),
        "the original DataForge input is encrypted"
    );
    for origin in [owner.origin(), row.position().origin()] {
        assert!(
            origin
                .source_uses_refs()
                .iter()
                .all(|reference| { inventory.iter_uses().any(|used| used.id() == reference) }),
            "all original contribution IDs resolve in the query inventory"
        );
    }
    Ok(())
}

fn refuse_non_ship(archive: &Archive, entry: &DataForgeEntry) -> TestResult {
    let refused = full_proof::from_entry(archive, entry, full_proof::NO_LANGUAGES, None)?
        .read_installed_configurations(UNSELECTED_RECORD, None)?;
    assert_eq!(
        refused.loadout_state,
        SourceValueState::Absent,
        "an Inventory Record cannot serve as a Ship root"
    );
    assert!(
        refused.rows.is_empty(),
        "the non-ship has no installed configuration projection"
    );
    assert!(
        refused.configuration_dependencies.is_empty(),
        "the refused root consults no flight configurations"
    );
    Ok(())
}
