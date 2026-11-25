// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::{env, slice};

use p4kx_export::catalog_queries::equipment::{self, LoadoutOwnerSnapshot};
use p4kx_export::catalog_queries::{CatalogRecordSelection, DatasetAvailability};
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
    if indices.is_empty() || indices.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("configure a nonempty, strictly ordered Loadout Record inventory".into());
    }
    let expected = Census {
        owners: indices.len(),
        rows: indices.len(),
        skipped: 0,
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
            "Loadout visitor SDK: route={route} availability={availability:?} owners={} rows={} skippedLeftovers={} maximumOwnerUses={} maximumOwnerPhysicalSources={}",
            expected.owners, expected.rows, expected.skipped, sources.0, sources.1,
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
    let summary = equipment::visit_loadouts(archive, selection, None, |snapshot| -> TestResult {
        let position = (snapshot.entity_guid.to_string(), snapshot.record_index);
        if previous.as_ref().is_some_and(|last| last >= &position) {
            return Err("Loadout owners are duplicated or outside canonical GUID order".into());
        }
        let row = snapshot.row.as_ref().map(wire_row).transpose()?;
        if let Some(row) = row {
            check_identity(snapshot, &row, &position.0)?;
            inspect(snapshot, row)?;
            observed.rows += 1;
        }
        previous = Some(position);
        observed.owners += 1;
        Ok(())
    })?;
    if summary.build.is_none() || summary.build.as_ref() != archive.build() {
        return Err("Loadout visitor returned a different Archive Build".into());
    }
    component_family_parity::require_complete(
        LOADOUTS,
        summary.availability,
        summary.row_count,
        &summary.coverage,
    )?;
    observed.skipped = summary.coverage.skipped_leftovers;
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
}

impl Census {
    fn parse(input: &str) -> TestResult<Self> {
        let counts = input
            .split(',')
            .map(str::parse::<usize>)
            .collect::<Result<Vec<_>, _>>()?;
        let [owners, rows, skipped] = counts.as_slice() else {
            return Err("configure exact Loadout owner, row and skipped counts".into());
        };
        if *rows == 0 || rows > owners {
            return Err(
                "configure a positive Loadout row count within the physical owner census".into(),
            );
        }
        Ok(Self {
            owners: *owners,
            rows: *rows,
            skipped: *skipped,
        })
    }
}
