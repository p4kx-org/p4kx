// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{self, LoadoutOwnerKind};
use p4kx_extraction::{Archive, DataForgeEntry};

use super::super::LOADOUTS;
use super::{Route, TestResult};

#[path = "../it/loadouts_fixture/mod.rs"]
mod fixture;

const SHIP_RECORD: u32 = 0;

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        fixture::write_archive(directory.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let result = equipment::read_loadouts_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(SHIP_RECORD),
        None,
    )?;
    let [row] = result.rows.as_slice() else {
        return Err("one selected original Loadout".into());
    };
    assert_eq!(row.record_index(), SHIP_RECORD, "physical Loadout owner");
    assert_eq!(row.entity_kind(), LoadoutOwnerKind::Ship, "original Ship");
    let port = row.port(&[0, 1]).ok_or("nested port occurrence")?;
    assert_eq!(
        port.name().value().map(String::as_str),
        Some("gun_2"),
        "the original nested port remains addressable"
    );
    assert!(
        result.source_inventory.use_count() > 0,
        "the loadout retains source uses"
    );
    Route::queried_rows(
        LOADOUTS,
        &result.rows,
        "typed selected Loadout and synthetic producer parity witnessed; complete installed producer parity open",
    )
}
