// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::visit_loadout_facility_placements_from;
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, TestEntity, TestEntry, test_key};
use serde_json::Value;

use super::{DOCKED_RECORD, SHIP_RECORD, TestResult, fixture, full_proof, normal_placements};

const EMPTY_SHIP: u32 = 5;

pub(super) fn compare(archive: &Archive, entry: &DataForgeEntry, normal: &[Value]) -> TestResult {
    let parsed = full_proof::from_entry(archive, entry, full_proof::NO_LANGUAGES, None)?;
    let mut owners = Vec::new();
    let mut rows = Vec::new();
    let summary = parsed.visit_loadout_facility_placements(None, |snapshot| -> TestResult {
        owners.push(snapshot.ship_record_index);
        for row in &snapshot.rows {
            assert_eq!(
                row.ship_record_index().value(),
                Some(&snapshot.ship_record_index),
                "the row is bound to its physical Ship callback"
            );
            assert!(
                row.loadout_position().is_some(),
                "the visit emits Loadout rows"
            );
            rows.push(serde_json::to_value(row)?);
        }
        Ok(())
    })?;
    let normal: Vec<_> = normal.iter().filter(|row| is_loadout(row)).collect();
    assert_eq!(
        owners,
        [SHIP_RECORD, DOCKED_RECORD],
        "the visit keeps each admitted Ship"
    );
    assert_eq!(
        summary.owner_count,
        owners.len(),
        "the terminal count covers every callback"
    );
    assert_eq!(
        summary.placement_count,
        rows.len(),
        "the terminal count covers every placement"
    );
    assert!(
        matches!(summary.availability, DatasetAvailability::Available),
        "the authored facility and inventory source families are available"
    );
    assert!(
        summary.incomplete_owner_count > 0,
        "the missing Loadout target is reported"
    );
    assert!(
        rows.iter().eq(normal),
        "canonical ordered rows and Full evidence are identical"
    );
    Ok(())
}

fn is_loadout(row: &Value) -> bool {
    row.pointer("/source/kind").and_then(Value::as_str) == Some("loadout")
}

#[test]
fn the_loadout_facility_visit_keeps_empty_roots_and_marker_values() -> TestResult {
    let (_directory, archive) = archive_with_empty_ship()?;
    let entry = DataForgeEntry::read(&archive)?;
    let normal = normal_placements(&archive, &entry)?;
    let parsed = full_proof::from_entry(&archive, &entry, full_proof::NO_LANGUAGES, None)?;
    let mut owners = Vec::new();
    let summary = parsed.visit_loadout_facility_placements(None, |snapshot| -> TestResult {
        owners.push(snapshot.ship_record_index);
        if snapshot.ship_record_index == EMPTY_SHIP {
            assert_eq!(
                snapshot.loadout_state,
                SourceValueState::Present,
                "the empty Ship has a Loadout"
            );
            assert!(
                snapshot.rows.is_empty(),
                "the empty Ship emitted no placement"
            );
        }
        Ok(())
    })?;
    assert_eq!(
        owners,
        [SHIP_RECORD, DOCKED_RECORD, EMPTY_SHIP],
        "empty roots still have a callback"
    );
    assert_eq!(
        summary.placement_count,
        normal.iter().filter(|row| is_loadout(row)).count()
    );
    let marker =
        visit_loadout_facility_placements_from::<CatalogError>(&archive, &entry, None, |owner| {
            for row in &owner.rows {
                assert_eq!(
                    row.ship_record_index().origin().contains_encrypted_source(),
                    Some(true)
                );
                assert!(
                    row.ship_record_index()
                        .origin()
                        .source_uses_refs()
                        .is_empty(),
                    "default marker inventory is elided"
                );
            }
            Ok(())
        })?;
    assert_eq!(
        marker.placement_count, summary.placement_count,
        "marker and Full rows cover the same scope"
    );
    Ok(())
}

#[test]
fn the_loadout_facility_visit_honors_visitor_cancellation() -> TestResult {
    let (_directory, archive) = archive_with_empty_ship()?;
    let entry = DataForgeEntry::read(&archive)?;
    let cancel = CancelToken::new();
    let mut calls = 0;
    let result = visit_loadout_facility_placements_from::<CatalogError>(
        &archive,
        &entry,
        Some(&cancel),
        |_| {
            calls += 1;
            cancel.cancel();
            Ok(())
        },
    );
    assert!(matches!(result, Err(CatalogError::Cancelled)));
    assert_eq!(
        calls, 1,
        "no additional owner is emitted after cancellation"
    );
    Ok(())
}

fn archive_with_empty_ship() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("facility-owner-visit.p4k");
    let ship = TestEntity::new(
        u128::MAX,
        "FixtureEmptyFacilityHost",
        "libs/foundry/records/entities/spaceships/fixture_empty_facility_host.xml",
    )
    .vehicle("", "")
    .record();
    let database = fixture::database().record(ship).build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    Ok((directory, archive))
}
