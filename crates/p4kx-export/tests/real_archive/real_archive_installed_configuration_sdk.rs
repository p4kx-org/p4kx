// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::resource_topology::PhysicalPort;
use p4kx_export::catalog_queries::equipment::{
    AlignmentDependencyState, InstalledConfigurationSnapshot, read_installed_configurations,
    read_installed_configurations_from,
};
use p4kx_extraction::{Archive, SourceValueState};
use serde_json::Value;

use super::{
    complete, dataforge, document, physical_filter, produce, record_index, report_scoped_rows,
    ship_row, with_archive,
};
use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};

const TABLE: &str = "/placements/installedConfigurations";
const SHIP_RECORD: &str = "P4KX_FLIGHT_SDK_RECORD";

#[test]
#[ignore = "real Archive: configure the installed Build, physical Ship and exact producer scope"]
fn installed_flight_matches_exact_scoped_normal_producer() -> TestResult {
    with_archive(|archive| {
        let index = record_index(SHIP_RECORD)?;
        let input = dataforge(archive)?;
        let snapshot = read_installed_configurations_from(archive, &input, index, None)
            .map_err(|_| "read the nominated SDK installed flight scope")?;
        let rows = selected_rows(archive, index, &snapshot)?;
        let filter = physical_filter(
            &input,
            "P4KX_FLIGHT_SDK_FILTER",
            "P4KX_FLIGHT_SDK_SELECTED_RECORDS",
            &required_records(index, &snapshot),
        )?;
        let output = produce(archive, &input, &filter)?;
        let catalog = document(archive, &output)?;
        ship_row(&catalog, index)?;
        let table = catalog
            .pointer(TABLE)
            .ok_or("the normal installed flight table is absent")?;
        let actual = table
            .get("rows")
            .and_then(Value::as_array)
            .ok_or("the normal installed flight rows are absent")?
            .iter()
            .filter(|row| {
                row.pointer("/shipRecordIndex/value")
                    .and_then(Value::as_u64)
                    == Some(u64::from(index))
            });
        if table.get("availability") != Some(&serde_json::to_value(snapshot.availability)?)
            || !rows.iter().eq(actual)
        {
            return Err("the SDK and normal installed flight rows or availability differ".into());
        }
        let mut origins = Origins::default();
        origins.extend(&snapshot.source_inventory, &rows)?;
        origins.verify_in_document(&catalog)?;
        report_scoped_rows("Installed flight", "parity", &rows)
    })
}

#[test]
#[ignore = "real Archive: configure the installed Build and physical Ship; measure a fresh process"]
fn installed_flight_cold_query() -> TestResult {
    with_archive(|archive| {
        let index = record_index(SHIP_RECORD)?;
        let snapshot = read_installed_configurations(archive, index, None)
            .map_err(|_| "read the cold SDK installed flight scope")?;
        let rows = selected_rows(archive, index, &snapshot)?;
        let mut origins = Origins::default();
        origins.extend(&snapshot.source_inventory, &rows)?;
        report_scoped_rows("Installed flight", "cold", &rows)
    })
}

fn selected_rows(
    archive: &Archive,
    index: u32,
    snapshot: &InstalledConfigurationSnapshot,
) -> TestResult<Vec<Value>> {
    if snapshot.build.as_ref() != archive.build()
        || !matches!(snapshot.availability, DatasetAvailability::Available)
        || snapshot.loadout_state != SourceValueState::Present
        || !complete(&snapshot.loadout_coverage)
        || !snapshot.loadout_dependencies.is_complete()
        || snapshot.configuration_dependencies.iter().any(|owner| {
            ![&owner.flight_controllers, &owner.ifcs, &owner.thrusters]
                .into_iter()
                .all(complete)
        })
    {
        return Err("the installed flight witness needs complete same-Build coverage".into());
    }
    let covered: BTreeSet<_> = snapshot
        .configuration_dependencies
        .iter()
        .map(|dependency| dependency.record_index)
        .collect();
    let mut paths = BTreeSet::new();
    let mut attached = false;
    let mut rows = Vec::new();
    for row in &snapshot.rows {
        let position = row
            .position()
            .value()
            .ok_or("the installed flight occurrence has no physical Loadout position")?;
        let owner = match row.attached_item() {
            Some(attachment) if !position.port_ordinals().is_empty() => {
                attached |= row.original_port().is_some();
                *attachment
                    .target_record_index()
                    .value()
                    .ok_or("the original attached flight owner is absent")?
            }
            None if position.port_ordinals().is_empty() => index,
            _ => return Err("the installation context differs from its position".into()),
        };
        if row.ship_record_index().value() != Some(&index)
            || position.loadout_record_index() != index
            || !paths.insert(position.port_ordinals())
            || !covered.contains(&owner)
            || row.configurations().is_empty()
            || row.configurations().iter().any(|link| {
                link.owner().record_index().value() != Some(&owner)
                    || link.owner().component_ordinal().value().is_none()
            })
        {
            return Err("the SDK row lacks a unique sourced occurrence and component owner".into());
        }
        rows.push(wire_row(row)?);
    }
    if !attached {
        return Err("nominate a Ship with a flight configuration on a physical item port".into());
    }
    Ok(rows)
}

fn required_records(index: u32, snapshot: &InstalledConfigurationSnapshot) -> BTreeSet<u32> {
    let mut required = BTreeSet::from([index]);
    required.extend(
        snapshot
            .configuration_dependencies
            .iter()
            .map(|dependency| dependency.record_index),
    );
    required.extend(
        snapshot
            .loadout_dependencies
            .alignment
            .iter()
            .filter_map(|dependency| {
                matches!(dependency.state, AlignmentDependencyState::Complete)
                    .then_some(dependency.record_index)
            }),
    );
    required.extend(
        snapshot
            .rows
            .iter()
            .filter_map(|row| row.original_port().map(PhysicalPort::owner_record_index)),
    );
    required
}
