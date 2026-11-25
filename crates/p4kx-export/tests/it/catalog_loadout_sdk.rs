// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{AlignmentRole, LoadoutOwnerKind, LoadoutSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Guid};
use serde_json::Value;

use crate::commodity_v3_fixture::TestResult;
use crate::loadouts_fixture::{self, ALPHA, EMPTY_LOADOUT};
use crate::loadouts_support;
use crate::sdk_full_proof as full_proof;

#[test]
fn direct_loadouts_match_the_producer_and_preserve_nested_occurrences() -> TestResult {
    let (_folder, archive) = fixture()?;
    let catalog: Value = serde_json::from_slice(
        build_catalog(
            &archive,
            loadouts_support::options(None).with_full_provenance(),
        )?
        .json(),
    )?;
    let snapshot = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
        .read_loadouts(CatalogRecordSelection::All, None)?;
    assert_eq!(
        serde_json::to_value(&snapshot.rows)?,
        catalog["loadouts"],
        "the selected SDK and Catalog render the same canonical loadout rows",
    );
    let alpha = snapshot
        .rows
        .iter()
        .find(|row| row.entity_guid().to_string() == loadouts_support::guid(ALPHA))
        .ok_or("exact root Ship")?;
    assert!(matches!(alpha.entity_kind(), LoadoutOwnerKind::Ship));
    let nested = alpha.port(&[0, 1]).ok_or("nested physical port")?;
    assert_eq!(nested.name().value().map(String::as_str), Some("gun_2"));
    let last = alpha
        .ports()
        .len()
        .checked_sub(1)
        .ok_or("two physical ports with one name")?;
    let (Some(first), Some(second)) = (alpha.port(&[last - 1]), alpha.port(&[last])) else {
        return Err("each original repeated port is accessible".into());
    };
    assert_eq!(
        first.name().value(),
        second.name().value(),
        "two port occurrences share an authored name"
    );
    assert!(
        !std::ptr::eq(first, second),
        "positions select separate occurrences"
    );
    assert!(alpha.port(&[last + 1]).is_none(), "unavailable occurrence");
    assert!(
        !snapshot.dependencies.is_complete(),
        "fixture retains deliberately unreadable resolver inputs"
    );
    Ok(())
}

#[test]
fn selected_loadouts_preserve_sparse_and_empty_coverage() -> TestResult {
    let (_folder, archive) = fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let first = record_index(&entry, ALPHA)?;
    let selected = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?
        .read_loadouts(CatalogRecordSelection::RecordIndex(first), None)?;
    assert_selected(&selected, first)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let empty_item = record_index(&entry, EMPTY_LOADOUT)?;
    for record_index in [empty_item, u32::MAX] {
        let empty =
            parsed.read_loadouts(CatalogRecordSelection::RecordIndex(record_index), None)?;
        assert!(
            empty.rows.is_empty() && empty.dependencies.alignment.is_empty(),
            "no loadout declared for this physical selection"
        );
    }
    let stop = CancelToken::new();
    stop.cancel();
    assert!(matches!(
        parsed.read_loadouts(CatalogRecordSelection::RecordIndex(first), Some(&stop)),
        Err(CatalogError::Cancelled)
    ));
    Ok(())
}

fn assert_selected(snapshot: &LoadoutSnapshot, index: u32) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one physical selected Loadout".into());
    };
    assert_eq!(
        row.record_index(),
        index,
        "the requested physical Record is returned"
    );
    assert!(
        snapshot.dependencies.alignment.iter().any(|dependency| {
            dependency.record_index == index && matches!(dependency.role, AlignmentRole::Port)
        }),
        "the actually read original port configuration enters source coverage",
    );
    assert!(
        snapshot.coverage.unreadable_bodies.is_empty(),
        "selected root body is readable"
    );
    assert!(
        snapshot.source_inventory.use_count() > 0,
        "actual uses from selected owners and dependencies"
    );
    Ok(())
}

fn record_index(entry: &DataForgeEntry, guid: u128) -> TestResult<u32> {
    let expected: Guid = loadouts_support::guid(guid).parse()?;
    entry
        .open()?
        .records()
        .find(|record| record.guid() == expected)
        .map(|record| record.index())
        .ok_or_else(|| "one original fixture Record".into())
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        loadouts_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    Ok((folder, archive))
}

#[path = "catalog_loadout_root_sdk.rs"]
mod root_identity;

#[path = "catalog_loadout_name_origins.rs"]
mod name_origins;

#[path = "catalog_loadout_visit_sdk.rs"]
mod visit;
