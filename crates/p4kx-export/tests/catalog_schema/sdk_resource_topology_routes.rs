// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{Route, TestResult};

#[path = "../it/catalog_topology_declarations_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 6]> {
    const REMAINING: &str = "synthetic producer parity witnessed; installed producer parity open";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("resource_topology.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::database().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let selection = CatalogRecordSelection::All;
    let projected = parsed.read_resource_topology_placements(0, None)?;
    let placements = projected.placements;
    Ok([
        parsed.with_resource_network_links(selection, None, |snapshot| {
            Route::queried_rows(
                "/declarations/resourceNetworkLinks",
                snapshot.rows(),
                "complete same-Build producer parity with nine-owner ATLS admission remains open",
            )
        })??,
        parsed.with_room_associations(selection, None, |snapshot| {
            Route::queried_rows("/declarations/roomAssociations", snapshot.rows(), None)
        })??,
        parsed.with_item_room_associations(selection, None, |snapshot| {
            Route::queried_rows("/declarations/itemRoomAssociations", snapshot.rows(), None)
        })??,
        Route::queried_rows(
            "/placements/resourceNetworkNodes",
            placements.resource_network_nodes().rows,
            REMAINING,
        )?,
        Route::queried_rows(
            "/placements/resourceNetworkLinkBindings",
            placements.resource_network_link_bindings().rows,
            REMAINING,
        )?,
        Route::queried_rows(
            "/placements/roomAssociationBindings",
            placements.room_association_bindings().rows,
            REMAINING,
        )?,
    ])
}
