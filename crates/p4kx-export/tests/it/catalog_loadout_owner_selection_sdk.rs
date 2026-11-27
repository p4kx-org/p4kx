// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::equipment::{
    LoadoutOwnerKind, LoadoutOwnerSelection, visit_loadout_owners_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::DataForgeEntry;

use super::{fixture, record_index, shared_guid_fixture, verify_docking};
use crate::commodity_v3_fixture::TestResult;
use crate::loadouts_fixture::{ALPHA, BROKEN, EMPTY_LOADOUT};
use crate::sdk_full_proof as full_proof;

#[test]
fn loadout_batch_retains_physical_ties_and_reports_only_absent_indices() -> TestResult {
    let (_folder, archive, _) = shared_guid_fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let indices = [2, u32::MAX, 0, 1, 2, u32::MAX, 10];
    let mut order = Vec::new();
    let summary = visit_loadout_owners_from::<CatalogError>(
        &archive,
        &entry,
        LoadoutOwnerSelection::RecordIndices(&indices),
        None,
        |snapshot| {
            assert!(
                snapshot.row.is_some(),
                "an admitted empty Ship still emits a row"
            );
            order.push(snapshot.record_index);
            Ok(())
        },
    )?;
    assert_eq!(
        order,
        [1, 0, 2],
        "canonical GUID order keeps physical ties and removes duplicate selections"
    );
    assert_eq!(
        summary.missing_record_indices,
        [10, u32::MAX],
        "missing input indices are sorted and distinct"
    );
    assert_eq!(
        summary.owner_count, 3,
        "absent requests are not admitted owners"
    );
    assert!(
        summary.dependencies_complete,
        "missing indices are independent of resolved dependencies"
    );
    Ok(())
}

#[test]
fn ship_loadout_visits_preserve_resolved_nested_stock_and_owner_failures() -> TestResult {
    let (_folder, archive) = fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let alpha = record_index(&entry, ALPHA)?;
    let broken = record_index(&entry, BROKEN)?;
    let session = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    let parsed = session.parsed(None)?;
    let expected = all_ship_rows(&parsed)?;
    let mut observed = Vec::new();
    let summary = parsed.visit_loadout_owners::<Box<dyn Error>>(
        LoadoutOwnerSelection::Ships,
        None,
        |owner| {
            assert!(
                matches!(owner.entity_kind, LoadoutOwnerKind::Ship),
                "standalone Items remain outside the root scope"
            );
            if owner.record_index == alpha {
                let row = owner.row.as_ref().ok_or("resolved stock Ship")?;
                let nested = row.port(&[0, 1]).ok_or("nested stock port")?;
                assert_eq!(
                    nested.name().value().map(String::as_str),
                    Some("gun_2"),
                    "nested mounted Item ports keep their occurrence paths"
                );
                verify_docking(row)?;
            }
            if owner.record_index == broken {
                assert!(
                    !owner.dependencies.is_complete(),
                    "the unreadable XML stays visible for the selected Ship"
                );
            }
            observed.push((
                owner.record_index,
                serde_json::to_value(&owner.row)?,
                owner.dependencies.is_complete(),
            ));
            Ok(())
        },
    )?;
    assert_eq!(
        observed, expected,
        "Ship roots match the ordered, resolved rows from the ordinary All visitor"
    );
    assert_eq!(
        summary.owner_count,
        expected.len(),
        "terminal owner count matches selected Ships"
    );
    assert!(
        !summary.dependencies_complete,
        "terminal coverage retains the selected Ship failure"
    );
    Ok(())
}

fn all_ship_rows(
    parsed: &ParsedCatalogQueries<'_>,
) -> TestResult<Vec<(u32, serde_json::Value, bool)>> {
    let mut rows = Vec::new();
    let summary =
        parsed.visit_loadouts::<Box<dyn Error>>(CatalogRecordSelection::All, None, |owner| {
            if matches!(owner.entity_kind, LoadoutOwnerKind::Ship) {
                rows.push((
                    owner.record_index,
                    serde_json::to_value(&owner.row)?,
                    owner.dependencies.is_complete(),
                ));
            }
            Ok(())
        })?;
    assert!(
        summary.owner_count > rows.len(),
        "the All visit includes standalone Items"
    );
    Ok(rows)
}

#[test]
fn loadout_batch_distinguishes_absent_indices_from_empty_item_rows() -> TestResult {
    let (_folder, archive) = fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let item = record_index(&entry, EMPTY_LOADOUT)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let indices = [u32::MAX, item];
    let summary = session.visit_loadout_owners::<CatalogError>(
        LoadoutOwnerSelection::RecordIndices(&indices),
        None,
        |owner| {
            assert_eq!(
                owner.record_index, item,
                "the batch still admits the original empty Item"
            );
            assert!(
                owner.row.is_none(),
                "the original empty Item does not produce a row"
            );
            assert!(
                owner.dependencies.is_complete(),
                "the empty Item has no failed resolution"
            );
            Ok(())
        },
    )?;
    assert_eq!(
        summary.missing_record_indices,
        [u32::MAX],
        "only the unknown physical index is missing"
    );
    assert_eq!(
        summary.owner_count, 1,
        "the empty Item is an admitted owner"
    );
    assert_eq!(
        summary.row_count, 0,
        "the empty Item contributes no canonical row"
    );
    Ok(())
}
