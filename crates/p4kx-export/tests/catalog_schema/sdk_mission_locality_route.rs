// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogRecordSelection, MissionLocalitySnapshot, SourceRecordReference,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, InputScope, SourceInventory, SourceLocation};
use serde_json::{Value, json};

use super::{Route, TestResult, reputation_fixture};

pub(super) const DATASET: &str = "/definitions/missionLocalities";

pub(super) fn read() -> TestResult<Route> {
    let folder = tempfile::tempdir()?;
    let path = reputation_fixture::write_archive(folder.path())?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(
            &archive,
            &entry,
            CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
        )?
        .json(),
    )?;
    let expected = catalog.pointer(DATASET).ok_or("canonical locality table")?;
    let index = serde_json::from_value::<u32>(
        expected
            .pointer("/rows/0/recordIndex/value")
            .ok_or("physical locality index")?
            .clone(),
    )?;
    full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?.with_mission_localities(CatalogRecordSelection::RecordIndex(index), None, |snapshot| {
            verify_slots(&snapshot, index)?;
            assert_eq!(
                &json!({"availability": snapshot.availability, "rows": snapshot.rows}),
                expected,
                "the selected physical locality matches the canonical Catalog table"
            );
            Route::queried_rows(
                DATASET,
                &snapshot.rows,
                "encrypted synthetic producer and source parity available; installed producer parity open",
            )
        })?
}

fn verify_slots(snapshot: &MissionLocalitySnapshot<'_>, index: u32) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one physical locality".into());
    };
    assert_eq!(
        row.header().record_index().value(),
        Some(&index),
        "the query preserves its selected physical locality"
    );
    let slots = row
        .available_locations()
        .value()
        .ok_or("authored location slots")?;
    let [first, missing, empty, _, repeated] = slots.as_slice() else {
        return Err("five original location slots".into());
    };
    assert!(empty.value().is_none(), "the empty slot remains in place");
    let first = first.value().ok_or("original reference")?;
    let repeated = repeated.value().ok_or("repeated original reference")?;
    assert_eq!(
        first.guid().value(),
        repeated.guid().value(),
        "both original occurrences retain their common GUID"
    );
    let missing = missing.value().ok_or("authored missing target")?;
    assert_eq!(
        serde_json::to_value(missing.status())?,
        json!("dangling"),
        "the authored reference survives with its resolution status"
    );
    verify_link_evidence(&snapshot.source_inventory, index, first)
}

fn verify_link_evidence(
    inventory: &SourceInventory,
    index: u32,
    link: &SourceRecordReference<'_>,
) -> TestResult {
    let guid = link.guid();
    let target_index = link.target_record_index();
    let owner_site = [u64::from(index), 1];
    let target = target_index
        .value()
        .copied()
        .ok_or("resolved target Record index")?;
    let target_site = [u64::from(target)];
    for (origin, namespace, positions, field) in [
        (
            guid.origin(),
            "dataforgeValue",
            owner_site.as_slice(),
            "$value",
        ),
        (
            target_index.origin(),
            "dataforgeRecordHeader",
            target_site.as_slice(),
            "recordIndex",
        ),
    ] {
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "the field comes from the encrypted entry"
        );
        assert!(
            origin.source_set_complete() && inventory.iter_uses().any(|used| {
                used.scope() == InputScope::Payload
                    && origin.source_uses_refs().contains(used.id())
                    && matches!(used.location(), SourceLocation::Subresource {
                        namespace: actual_namespace, indices, field: actual_field
                    } if actual_namespace == namespace && indices.as_slice() == positions && actual_field == field)
            }),
            "the input inventory preserves distinct body and target-header evidence"
        );
    }
    Ok(())
}
