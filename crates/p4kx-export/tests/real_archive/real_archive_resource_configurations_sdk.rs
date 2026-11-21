// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::component_definitions::ComponentDefinitionSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_table, verify_tables,
};
use crate::real_archive_verification::print_line;

pub(super) const TABLES: [&str; 7] = [
    "/configurations/turretHealth",
    "/configurations/radarOptions",
    "/definitions/resourceNetworkGlobals",
    "/configurations/resourceNetworkParameters",
    "/declarations/resourceNetworkLinks",
    "/declarations/roomAssociations",
    "/declarations/itemRoomAssociations",
];

#[test]
#[ignore = "same Build: set P4KX_DATA_P4K and P4KX_CATALOG_MEASUREMENT_DIR"]
fn installed_resource_configuration_sdk_matches_one_complete_catalog() -> TestResult {
    let counts = verify_tables(TABLES, read_sdk)?;
    let [
        turrets,
        radar,
        globals,
        network,
        links,
        vehicle_rooms,
        item_rooms,
    ] = counts.rows;
    print_line(&format!(
        "complete Catalog SDK parity: turretHealthRows={turrets} radarOptionsRows={radar} resourceNetworkGlobalsRows={globals} resourceNetworkParametersRows={network} resourceNetworkLinksRows={links} roomAssociationsRows={vehicle_rooms} itemRoomAssociationsRows={item_rooms} sourceUses={} physicalSources={}",
        counts.source_uses, counts.physical_sources,
    ));
    Ok(())
}

pub(super) fn read_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 7]> {
    let session =
        CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let all = CatalogRecordSelection::All;
    let turrets = parsed.with_turret_health(all, None, |snapshot| {
        capture_table(&snapshot, archive, origins, component_identity)
    })??;
    let radar = parsed.with_radar_options(all, None, |snapshot| {
        capture_game_mode(&snapshot, archive, origins)
    })??;
    let globals = parsed.with_resource_network_globals(all, None, |snapshot| {
        capture_table(&snapshot, archive, origins, |row| {
            index(row, "/recordIndex/value")
        })
    })??;
    let network = parsed.with_resource_network_parameters(all, None, |snapshot| {
        capture_game_mode(&snapshot, archive, origins)
    })??;
    let links = parsed.with_resource_network_links(all, None, |snapshot| {
        capture_table(&snapshot, archive, origins, component_identity)
    })??;
    let vehicle_rooms = parsed.with_room_associations(all, None, |snapshot| {
        capture_table(&snapshot, archive, origins, component_identity)
    })??;
    let item_rooms = parsed.with_item_room_associations(all, None, |snapshot| {
        capture_table(&snapshot, archive, origins, component_identity)
    })??;
    Ok([
        turrets,
        radar,
        globals,
        network,
        links,
        vehicle_rooms,
        item_rooms,
    ])
}

fn capture_game_mode<Row: Serialize>(
    snapshot: &ComponentDefinitionSnapshot<'_, Row>,
    archive: &Archive,
    origins: &mut Origins,
) -> TestResult<Table> {
    if !snapshot.definition_discovery().is_complete() {
        return Err("the SDK must complete the original GameMode definition discovery".into());
    }
    capture_table(snapshot, archive, origins, |row| {
        index(row, "/source/recordIndex/value")
    })
}

fn component_identity(row: &Value) -> TestResult<(u64, u64)> {
    Ok((
        index(row, "/ownerRecordIndex/value")?,
        index(row, "/componentOrdinal/value")?,
    ))
}

fn index(row: &Value, pointer: &str) -> TestResult<u64> {
    row.pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("required original physical row identity at {pointer}").into())
}
