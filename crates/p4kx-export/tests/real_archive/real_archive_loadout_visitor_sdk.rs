// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::{env, slice};

use p4kx_export::catalog_queries::equipment::LoadoutOwnerSnapshot;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability,
};
use p4kx_extraction::{Archive, Guid};
use serde_json::Value;

use crate::real_archive_catalog_table_parity::{
    DefinitionOwner, Origins, StreamedDefinitionDigests, TestResult, wire_row,
    with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::{component_family_parity, equipment_input};
use crate::real_archive_verification::print_line;

const LOADOUTS: &str = "/loadouts";
const PRODUCER: &str = "P4KX_LOADOUT_CATALOG_PRODUCER_REVISION";
const CENSUS: &str = "P4KX_LOADOUT_OWNER_CENSUS";
const ANCHOR_GUID: &str = "P4KX_LOADOUT_WITNESS_GUID";
const GUID_INDICES: &str = "P4KX_LOADOUT_GUID_INDICES";
const RECORD_INDEX: &str = "P4KX_LOADOUT_RECORD_INDEX";

#[test]
#[ignore = "installed Archive: configure the exact Build and sealed Loadout census"]
fn installed_loadout_visitor_cold_all() -> TestResult {
    let expected = Census::parse(&env::var(CENSUS)?)?;
    cold_visit("all", CatalogRecordSelection::All, expected, |_| Ok(()))
}

#[test]
#[ignore = "installed Archive: configure the exact Build and a physically verified GUID witness"]
fn installed_loadout_visitor_cold_guid() -> TestResult {
    let guid: Guid = env::var(ANCHOR_GUID)?.parse()?;
    let indices = env::var(GUID_INDICES)?
        .split(',')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()?;
    cold_narrow("guid", CatalogRecordSelection::Guid(guid), guid, &indices)
}

#[test]
#[ignore = "installed Archive: configure the exact Build and a physically verified Record witness"]
fn installed_loadout_visitor_cold_record_index() -> TestResult {
    let guid: Guid = env::var(ANCHOR_GUID)?.parse()?;
    let index: u32 = env::var(RECORD_INDEX)?.parse()?;
    cold_narrow(
        "record-index",
        CatalogRecordSelection::RecordIndex(index),
        guid,
        &[index],
    )
}

#[test]
#[ignore = "same Build: configure the sealed census and complete Normal Catalog"]
fn installed_loadout_visitor_matches_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected = Census::parse(&env::var(CENSUS)?)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut evidence =
            StreamedDefinitionDigests::new(scratch.path(), [DefinitionOwner::Loadout]);
        let availability = visit(
            archive,
            CatalogRecordSelection::All,
            expected,
            |snapshot, row| evidence.retain_visited_row(0, row, &snapshot.source_inventory),
        )?;
        evidence.record_availability(0, availability)?;
        let counts = evidence.compare_arrays(measurement, [LOADOUTS])?;
        if counts.rows != [expected.rows] {
            return Err("the retained Catalog differs from the terminal Loadout row census".into());
        }
        print_line(&format!(
            "complete Loadout visitor parity: rows={} sourceUses={} physicalSources={}",
            counts.rows[0], counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

fn cold_narrow(
    route: &str,
    selection: CatalogRecordSelection,
    guid: Guid,
    indices: &[u32],
) -> TestResult {
    if indices.is_empty() || !indices.is_sorted_by(|left, right| left < right) {
        return Err("configure a nonempty, strictly ordered Loadout Record inventory".into());
    }
    let expected = Census {
        owners: indices.len(),
        rows: indices.len(),
        skipped: 0,
        rows_with_issues: 0,
    };
    let mut offset = 0;
    cold_visit(route, selection, expected, |owner| {
        if owner.entity_guid != guid || indices.get(offset) != Some(&owner.record_index) {
            return Err("narrow Loadout visitor returned a different physical owner".into());
        }
        offset += 1;
        Ok(())
    })
}

fn cold_visit(
    route: &str,
    selection: CatalogRecordSelection,
    expected: Census,
    mut check_owner: impl FnMut(&LoadoutOwnerSnapshot) -> TestResult,
) -> TestResult {
    equipment_input::with_archive(|archive| {
        let mut sources = (0, 0);
        let availability = visit(archive, selection, expected, |snapshot, row| {
            check_owner(snapshot)?;
            let mut origins = Origins::default();
            origins.extend(&snapshot.source_inventory, slice::from_ref(&row))?;
            let (uses, physical) = origins.contributing_counts();
            if uses == 0 || physical == 0 {
                return Err("an emitted Loadout lacks original contributing provenance".into());
            }
            sources = (sources.0.max(uses), sources.1.max(physical));
            Ok(())
        })?;
        print_line(&format!(
            "Loadout visitor SDK: route={route} availability={availability:?} owners={} rows={} skippedLeftovers={} rowsWithIssues={} maximumOwnerUses={} maximumOwnerPhysicalSources={}",
            expected.owners,
            expected.rows,
            expected.skipped,
            expected.rows_with_issues,
            sources.0,
            sources.1,
        ));
        Ok(())
    })
}

fn visit(
    archive: &Archive,
    selection: CatalogRecordSelection,
    expected: Census,
    mut inspect: impl FnMut(&LoadoutOwnerSnapshot, Value) -> TestResult,
) -> TestResult<DatasetAvailability> {
    let mut observed = Census::default();
    let mut previous = None;
    let mut reported_issue_owners = Vec::new();
    let session =
        equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
    let summary = session.visit_loadouts(selection, None, |snapshot| -> TestResult {
        let position = (snapshot.entity_guid.to_string(), snapshot.record_index);
        if previous.as_ref().is_some_and(|last| last >= &position) {
            return Err("Loadout owners are duplicated or outside canonical GUID order".into());
        }
        if !snapshot.dependencies.is_complete() {
            return Err(format!(
                "Loadout owner {} has incomplete resolver or alignment dependencies",
                snapshot.record_index,
            )
            .into());
        }
        if let Some(loadout) = &snapshot.row {
            let row = wire_row(loadout)?;
            check_identity(snapshot, &row, &position.0)?;
            inspect(snapshot, row)?;
            observed.rows += 1;
            if !loadout.issues().is_empty() {
                reported_issue_owners.push(snapshot.record_index);
            }
        }
        previous = Some(position);
        observed.owners += 1;
        Ok(())
    })?;
    if summary.build.is_none() || summary.build.as_ref() != archive.build() {
        return Err("Loadout visitor returned a different Archive Build".into());
    }
    reported_issue_owners.sort_unstable();
    component_family_parity::require_reported_fields(
        LOADOUTS,
        summary.availability,
        summary.row_count,
        &summary.coverage,
        &reported_issue_owners,
    )?;
    observed.skipped = summary.coverage.skipped_leftovers;
    observed.rows_with_issues = reported_issue_owners.len();
    if summary.owner_count != observed.owners
        || summary.row_count != observed.rows
        || !summary.dependencies_complete
        || observed != expected
    {
        return Err(format!(
            "incomplete Loadout witness, expected {expected:?}, observed {observed:?}"
        )
        .into());
    }
    Ok(summary.availability)
}

fn check_identity(snapshot: &LoadoutOwnerSnapshot, row: &Value, guid: &str) -> TestResult {
    let entity_kind = wire_row(&snapshot.entity_kind)?;
    if DefinitionOwner::Loadout.physical_index(row)? != snapshot.record_index
        || row.pointer("/entityGuid/value").and_then(Value::as_str) != Some(guid)
        || row.pointer("/entityKind/value") != Some(&entity_kind)
    {
        return Err("Loadout wire row differs from the original owner identity".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Census {
    owners: usize,
    rows: usize,
    skipped: usize,
    rows_with_issues: usize,
}

impl Census {
    fn parse(input: &str) -> TestResult<Self> {
        let counts = input
            .split(',')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()?;
        let [owners, rows, skipped, rows_with_issues] = counts.as_slice() else {
            return Err("configure exact Loadout owner, row, skipped and issue-row counts".into());
        };
        if *rows == 0 || rows > owners || rows_with_issues > rows {
            return Err(
                "configure positive Loadout rows within the owner census and issue rows within all rows".into(),
            );
        }
        Ok(Self {
            owners: *owners,
            rows: *rows,
            skipped: *skipped,
            rows_with_issues: *rows_with_issues,
        })
    }
}

#[test]
fn census_requires_an_independent_issue_count() -> TestResult {
    assert_eq!(
        Census::parse("4,3,0,2")?.rows_with_issues,
        2,
        "the fourth count describes loadout rows with represented issues",
    );
    assert!(
        Census::parse("4,3,0").is_err(),
        "the broad census requires an explicit issue-row count",
    );
    assert!(
        Census::parse("4,3,0,4").is_err(),
        "issue owners cannot exceed emitted rows",
    );
    Ok(())
}

#[test]
fn visitor_verifies_a_represented_issue_and_rejects_an_unreadable_dependency() -> TestResult {
    let expected = Census {
        owners: 1,
        rows: 1,
        skipped: 0,
        rows_with_issues: 1,
    };
    let (_folder, archive) = issue_fixture(false)?;
    visit(&archive, CatalogRecordSelection::All, expected, |_, row| {
        assert_eq!(
            row.get("issues"),
            Some(&serde_json::json!(["unknown_class"])),
            "a missing attachment target is represented by the canonical row",
        );
        Ok(())
    })?;

    let (_folder, archive) = issue_fixture(true)?;
    let result = visit(&archive, CatalogRecordSelection::All, expected, |_, _| {
        Ok(())
    });
    assert!(
        result.is_err_and(|error| error
            .to_string()
            .contains("incomplete resolver or alignment")),
        "an incomplete original dependency blocks an issue-bearing row",
    );
    Ok(())
}

fn issue_fixture(malformed_vehicle: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    use p4kx_testkit::{
        ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
        TestPort, entity_schema, write_build_manifest,
    };

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    write_build_manifest(folder.path(), "fixture", "1.2.3", "456", "issue-fixture")?;
    let mut ship = TestEntity::new(
        0x10,
        "ACME_Issue",
        "libs/foundry/records/entities/spaceships/acme_issue.xml",
    )
    .port(TestPort::new("slot"))
    .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
        "slot", "Unknown",
    )]));
    let mut input = ArchiveBuilder::new();
    if malformed_vehicle {
        let vehicle = "Scripts/Entities/Vehicles/Implementations/Xml/ACME_Issue.xml";
        ship = ship.vehicle(vehicle, "");
        input = input.entry(TestEntry::zstd(
            format!("Data/{vehicle}"),
            b"CryXmlB\0not really".to_vec(),
        ));
    }
    let forge = entity_schema(DataForgeBuilder::new()).record(ship.record());
    input
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    Ok((folder, archive))
}
