// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_export::catalog_queries::equipment::{Loadout, LoadoutOwnerSnapshot, visit_loadouts_from};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Guid};
use p4kx_testkit::{DataForgeBuilder, TestEntity, entity_schema};
use serde_json::{Value, json};

use super::{fixture, record_index};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as source_checks;
use crate::loadouts_fixture::{ALPHA, BROKEN, EMPTY_LOADOUT, SNUB};
use crate::loadouts_support;
use crate::sdk_full_proof as full_proof;

#[test]
fn owned_loadout_visits_match_catalog_rows_and_original_sources() -> TestResult {
    let (_folder, archive) = fixture()?;
    let catalog: Value = serde_json::from_slice(
        build_catalog(
            &archive,
            loadouts_support::options(None).with_full_provenance(),
        )?
        .json(),
    )?;
    let rows = catalog["loadouts"].as_array().ok_or("Catalog Loadouts")?;
    let mut expected = rows.iter();
    let mut malformed_seen = false;
    let result = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
        .visit_loadouts::<Box<dyn Error>>(CatalogRecordSelection::All, None, |snapshot| {
            if let Some(row) = &snapshot.row {
                let wire = serde_json::to_value(row)?;
                assert_eq!(Some(&wire), expected.next(), "canonical row and order");
                source_checks::closure(
                    &json!({"loadouts": [wire], "provenance": snapshot.source_inventory}),
                )?;
                verify_docking(row)?;
            }
            assert_owner_scope(snapshot)?;
            if snapshot.entity_guid == loadouts_support::guid(BROKEN).parse::<Guid>()? {
                malformed_seen = true;
                assert!(
                    !snapshot.dependencies.is_complete(),
                    "the invalid vehicle XML leaves a scoped dependency failure"
                );
            }
            Ok(())
        })?;
    assert!(expected.next().is_none(), "all canonical rows were visited");
    assert_eq!(result.row_count, rows.len(), "the terminal row count");
    assert!(
        result.owner_count >= result.row_count,
        "selected owner census"
    );
    assert!(malformed_seen, "the broken Ship remains visible");
    assert!(
        !result.dependencies_complete,
        "owner dependency errors remain visible in the terminal summary"
    );
    Ok(())
}

fn verify_docking(row: &Loadout) -> TestResult {
    let owner = row.entity_guid().to_string();
    if owner == loadouts_support::guid(ALPHA) {
        let dock = row
            .ports()
            .iter()
            .find(|port| port.name().value().is_some_and(|name| name == "hangar"))
            .ok_or("original dock port")?;
        assert!(
            dock.docked_vehicle() && dock.ports().is_empty(),
            "a docked vehicle's own Loadout is deferred"
        );
    }
    if owner == loadouts_support::guid(SNUB) {
        assert!(
            row.ports()
                .iter()
                .any(|port| port.name().value().is_some_and(|name| name == "snub_gun")),
            "the deferred vehicle can be read as its own owner"
        );
    }
    Ok(())
}

fn assert_owner_scope(snapshot: &LoadoutOwnerSnapshot) -> TestResult {
    let owner = snapshot.entity_guid.to_string();
    let owns_vehicle = owner == loadouts_support::guid(ALPHA);
    if owns_vehicle || owner == loadouts_support::guid(SNUB) {
        let inventory = serde_json::to_value(&snapshot.source_inventory)?;
        let vehicle_observed = source_checks::rows(&inventory, "/sources")?
            .iter()
            .filter_map(|source| source.pointer("/path/value").and_then(Value::as_str))
            .filter_map(|path| path.rsplit(['\\', '/']).next())
            .any(|name| name.eq_ignore_ascii_case("ACME_Alpha.xml"));
        assert_eq!(
            vehicle_observed, owns_vehicle,
            "vehicle XML remains exclusive to the owner whose resolver read it"
        );
    }
    Ok(())
}

#[test]
fn owned_loadout_visits_keep_guid_ties_and_exact_record_selection() -> TestResult {
    let (_folder, archive, guid) = shared_guid_fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let cases: [(CatalogRecordSelection, &[u32]); 3] = [
        (CatalogRecordSelection::All, &[1, 0, 2]),
        (CatalogRecordSelection::Guid(guid), &[0, 2]),
        (CatalogRecordSelection::RecordIndex(2), &[2]),
    ];
    for (selection, expected) in cases {
        let mut order = Vec::new();
        let summary =
            visit_loadouts_from::<CatalogError>(&archive, &entry, selection, None, |snapshot| {
                assert!(snapshot.row.is_some(), "an empty Ship has a Loadout row");
                order.push(snapshot.record_index);
                Ok(())
            })?;
        assert_eq!(
            order.as_slice(),
            expected,
            "GUID order and original physical ties"
        );
        assert_eq!(summary.row_count, expected.len(), "every selected Ship");
        assert!(summary.dependencies_complete, "empty valid Ships resolve");
    }
    Ok(())
}

fn shared_guid_fixture() -> TestResult<(tempfile::TempDir, Archive, Guid)> {
    const SHARED: u128 = 0xa1;
    let guid = loadouts_support::guid(SHARED).parse()?;
    let forge = [
        (SHARED, "ACME_First"),
        (0x10, "ACME_Early"),
        (SHARED, "ACME_Second"),
    ]
    .into_iter()
    .fold(
        entity_schema(DataForgeBuilder::new()),
        |builder, (key, name)| {
            builder.record(
                TestEntity::new(
                    key,
                    name,
                    format!("libs/foundry/records/entities/spaceships/{name}.xml"),
                )
                .record(),
            )
        },
    );
    let (folder, archive) = common::open(&forge, DcbOrigin::Encrypted)?;
    Ok((folder, archive, guid))
}

#[derive(Debug)]
enum Stop {
    Visitor,
    Catalog(CatalogError),
}

impl From<CatalogError> for Stop {
    fn from(error: CatalogError) -> Self {
        Self::Catalog(error)
    }
}

#[test]
fn owned_loadout_visits_stop_on_visitor_error_or_cancellation() -> TestResult {
    let (_folder, archive) = fixture()?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let mut callbacks = 0;
    let rejected = session.visit_loadouts(CatalogRecordSelection::All, None, |_| {
        callbacks += 1;
        Err(Stop::Visitor)
    });
    assert!(
        matches!(rejected, Err(Stop::Visitor)),
        "original callback error"
    );
    assert_eq!(callbacks, 1, "failure stops after one owner");

    let parsed = session.parsed(None)?;
    let token = CancelToken::new();
    let mut callbacks = 0;
    let cancelled =
        parsed.visit_loadouts::<Stop>(CatalogRecordSelection::All, Some(&token), |_| {
            callbacks += 1;
            token.cancel();
            Ok(())
        });
    assert!(
        matches!(cancelled, Err(Stop::Catalog(CatalogError::Cancelled))),
        "cancellation prevents a final summary"
    );
    assert_eq!(callbacks, 1, "cancellation prevents later callbacks");
    Ok(())
}

#[test]
fn admitted_empty_item_has_no_row_and_absent_selection_has_no_owner() -> TestResult {
    let (_folder, archive) = fixture()?;
    let entry = DataForgeEntry::read(&archive)?;
    let item = record_index(&entry, EMPTY_LOADOUT)?;
    for (index, expected) in [(item, Some(item)), (u32::MAX, None)] {
        let mut seen = None;
        let summary = visit_loadouts_from::<CatalogError>(
            &archive,
            &entry,
            CatalogRecordSelection::RecordIndex(index),
            None,
            |snapshot| {
                assert!(
                    snapshot.row.is_none(),
                    "a resolved Item without ports has no row"
                );
                assert!(
                    snapshot.dependencies.resolver.failed_records().is_empty()
                        && snapshot.dependencies.resolver.failed_entries().is_empty(),
                    "the authored empty case is distinct from a failed source read"
                );
                assert!(
                    seen.replace(snapshot.record_index).is_none(),
                    "one exact owner"
                );
                Ok(())
            },
        )?;
        assert_eq!(
            seen, expected,
            "known empty Item and absent selection differ"
        );
        assert_eq!(summary.row_count, 0, "no canonical row was delivered");
        assert_eq!(
            summary.owner_count,
            usize::from(expected.is_some()),
            "only the authored empty owner reached the resolver"
        );
    }
    Ok(())
}
