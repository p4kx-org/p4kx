// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordCoverage;
use p4kx_export::catalog_queries::equipment::{
    AlignmentDependencyState, LoadoutFacilityPlacementSnapshot, ShipStoreSlugSnapshot,
    read_loadout_facility_placements, read_loadout_facility_placements_from, with_ship_store_slugs,
    with_ship_store_slugs_from,
};
use p4kx_export::{BuiltCatalog, CatalogOptions, SCHEMA_VERSION, build_catalog_from};
use p4kx_extraction::{Archive, Checksum, DataForgeEntry, Filter, SourceValueState};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};
use crate::real_archive_shop_inventory_v3::verify_build;
use crate::real_archive_verification::print_line;

#[path = "real_archive_equipment_input.rs"]
mod equipment_input;
use equipment_input::{dataforge, literal_filter, physical_filter, record_index, with_archive};

#[path = "vehicle_armor_sdk_cold.rs"]
mod vehicle_armor_cold;

#[path = "real_archive_standalone_mining_sdk_cold.rs"]
mod mining_cold;

#[path = "real_archive_location_vocabulary_sdk_parity.rs"]
mod location_vocabulary_parity;

#[path = "real_archive_loot_definition_sdk_parity.rs"]
mod loot_definition_parity;

#[path = "real_archive_jump_configuration_sdk_parity.rs"]
mod jump_configuration_parity;

#[path = "real_archive_resource_group_sdk_parity.rs"]
mod resource_group_parity;

#[path = "real_archive_terrain_definition_sdk_parity.rs"]
mod terrain_definition_parity;

#[path = "real_archive_npc_spawn_definition_sdk_parity.rs"]
mod npc_spawn_definition_parity;

#[path = "real_archive_canonical_ship_sdk.rs"]
mod canonical_ship;

#[path = "real_archive_canonical_item_variant_sdk.rs"]
mod canonical_item;

#[path = "real_archive_variant_parity.rs"]
mod variant_parity;

#[path = "real_archive_installed_configuration_sdk.rs"]
mod installed_configuration;

#[path = "real_archive_store_slug_retained_parity.rs"]
mod store_slug_retained_parity;

#[path = "real_archive_physical_shop_sdk.rs"]
mod physical_shop;

#[path = "real_archive_paint_compatibility.rs"]
mod paint_compatibility;
#[path = "real_archive_selected_weapon_sdk.rs"]
mod selected_weapon;

const STORE: &str = "/shipStoreSlugs";
const FACILITY: &str = "/placements/facilityInstances/rows";
const STORE_RECORD: &str = "P4KX_STORE_SLUG_SDK_RECORD";
const FACILITY_RECORD: &str = "P4KX_FACILITY_SDK_RECORD";

#[test]
#[ignore = "real Archive: configure the installed Build and one physical mapped Ship Record"]
fn installed_store_slugs_match_exact_scoped_normal_producer() -> TestResult {
    with_archive(|archive| {
        let index = record_index(STORE_RECORD)?;
        let input = dataforge(archive)?;
        let filter = literal_filter(&input, index)?;
        let output = produce(archive, &input, &filter)?;
        let document = document(archive, &output)?;
        let original = ship_row(&document, index)?;
        if original
            .pointer("/playability/value")
            .and_then(Value::as_str)
            != Some("player")
        {
            return Err("the nominated original Ship is not classified as player".into());
        }
        let class = original
            .pointer("/className/value")
            .and_then(Value::as_str)
            .ok_or("the original Ship class is absent")?;
        let guid = original
            .pointer("/guid/value")
            .and_then(Value::as_str)
            .ok_or("the original Ship GUID is absent")?
            .parse()
            .map_err(|_| "the original Ship GUID is invalid")?;
        with_ship_store_slugs_from(archive, &input, Some(&filter), None, |snapshot| {
            store_selection(archive, &snapshot, Some(&filter))?;
            let mut bindings = snapshot
                .rows()
                .iter()
                .filter(|row| row.ship_guid() == Some(guid) && row.class_name() == Some(class))
                .peekable();
            if bindings.peek().is_none() {
                return Err("the nominated physical Ship has no admitted Store slug".into());
            }
            if document.pointer(STORE) != Some(&serde_json::to_value(snapshot.rows())?)
                || output.summary().store_slugs() != snapshot.summary()
                || document.pointer("/completeness/shipStoreSlugs")
                    != Some(&json!({"status": "partial", "filter": filter.as_str()}))
            {
                return Err(
                    "the scoped Store rows, summary or selection differ from the producer".into(),
                );
            }
            for binding in bindings {
                print_line(&format!(
                    "Store slug scoped parity rsi_id={} slug={} guid={guid} class={class}",
                    binding.rsi_id(),
                    binding.slug(),
                ));
            }
            print_line(&format!(
                "Store slug scoped parity rows={}",
                snapshot.rows().len()
            ));
            Ok(())
        })
        .map_err(|_| "read the SDK Store scope")?
    })
}

#[test]
#[ignore = "real Archive: configure the installed Build; run in its own measured process"]
fn installed_store_slugs_cold_unfiltered_query() -> TestResult {
    with_archive(|archive| {
        with_ship_store_slugs(archive, None, None, |snapshot| {
            store_selection(archive, &snapshot, None)?;
            print_line(&format!(
                "Store slug unfiltered cold rows={}",
                snapshot.rows().len()
            ));
            Ok(())
        })
        .map_err(|_| "read the unfiltered SDK Store scope")?
    })
}

#[test]
#[ignore = "real Archive: configure the installed Build, physical Ship index and exact producer scope"]
fn installed_loadout_facilities_match_exact_scoped_normal_producer() -> TestResult {
    with_archive(|archive| {
        let index = record_index(FACILITY_RECORD)?;
        let input = dataforge(archive)?;
        let snapshot = read_loadout_facility_placements_from(archive, &input, index, None)
            .map_err(|_| "read the nominated SDK Loadout Facility scope")?;
        let rows = facility_rows(archive, index, &snapshot)?;
        let filter = facility_filter(&input, index, &snapshot)?;
        let output = produce(archive, &input, &filter)?;
        let document = document(archive, &output)?;
        ship_row(&document, index)?;
        let actual = document
            .pointer(FACILITY)
            .and_then(Value::as_array)
            .ok_or("the normal Facility Placement table is absent")?
            .iter()
            .filter(|row| {
                row.pointer("/source/kind").and_then(Value::as_str) == Some("loadout")
                    && row
                        .pointer("/shipRecordIndex/value")
                        .and_then(Value::as_u64)
                        == Some(u64::from(index))
            });
        if !rows.iter().eq(actual) {
            return Err("the exact SDK and normal producer Facility rows differ".into());
        }
        let mut origins = Origins::default();
        origins.extend(&snapshot.source_inventory, &rows)?;
        origins.verify_in_document(&document)?;
        report_scoped_rows("Facility", "parity", &rows)
    })
}

#[test]
#[ignore = "real Archive: configure the installed Build and physical Ship index; run in its own measured process"]
fn installed_loadout_facility_cold_query() -> TestResult {
    with_archive(|archive| {
        let index = record_index(FACILITY_RECORD)?;
        let snapshot = read_loadout_facility_placements(archive, index, None)
            .map_err(|_| "read the cold SDK Loadout Facility scope")?;
        let rows = facility_rows(archive, index, &snapshot)?;
        let mut origins = Origins::default();
        origins.extend(&snapshot.source_inventory, &rows)?;
        report_scoped_rows("Facility", "cold", &rows)
    })
}

fn report_scoped_rows(kind: &str, phase: &str, rows: &[Value]) -> TestResult {
    let digest = scoped_rows_digest(rows)?;
    print_line(&format!(
        "{kind} {phase} rows={} digest={digest}",
        rows.len()
    ));
    Ok(())
}

fn scoped_rows_digest(rows: &[Value]) -> TestResult<String> {
    Ok(Checksum::Sha256(Sha256::digest(serde_json::to_vec(rows)?).into()).hex())
}

fn facility_rows(
    archive: &Archive,
    index: u32,
    snapshot: &LoadoutFacilityPlacementSnapshot,
) -> TestResult<Vec<Value>> {
    if snapshot.build.as_ref() != archive.build()
        || snapshot.loadout_state != SourceValueState::Present
        || snapshot.rows.is_empty()
        || !complete(&snapshot.loadout_coverage)
        || !snapshot.loadout_dependencies.is_complete()
        || snapshot
            .configuration_dependencies
            .iter()
            .any(|dependency| {
                !matches!(
                    dependency.state,
                    AlignmentDependencyState::Complete | AlignmentDependencyState::NotAdmitted
                )
            })
    {
        return Err("the installed Facility SDK witness needs a non-empty complete same-Build Loadout scope".into());
    }
    let mut paths = BTreeSet::new();
    let mut rows = Vec::new();
    for row in &snapshot.rows {
        let position = row
            .loadout_position()
            .and_then(|position| position.value())
            .ok_or("the original Loadout position is missing")?;
        if row.ship_record_index().value() != Some(&index)
            || position.loadout_record_index() != index
            || !paths.insert(position.port_ordinals())
        {
            return Err("the SDK row lacks a unique path on the physical Ship".into());
        }
        rows.push(wire_row(row)?);
    }
    Ok(rows)
}

fn store_selection(
    archive: &Archive,
    snapshot: &ShipStoreSlugSnapshot<'_>,
    filter: Option<&Filter>,
) -> TestResult {
    if snapshot.build() != archive.build()
        || snapshot.filter() != filter.map(Filter::as_str)
        || !complete(snapshot.coverage())
        || snapshot.rows().is_empty()
        || snapshot.summary().exported != snapshot.rows().len()
    {
        return Err(
            "the installed Store SDK witness needs complete non-empty same-Build coverage".into(),
        );
    }
    Ok(())
}

fn facility_filter(
    input: &DataForgeEntry,
    index: u32,
    snapshot: &LoadoutFacilityPlacementSnapshot,
) -> TestResult<Filter> {
    physical_filter(
        input,
        "P4KX_FACILITY_SDK_FILTER",
        "P4KX_FACILITY_SDK_SELECTED_RECORDS",
        &facility_dependencies(index, snapshot)?,
    )
}

fn facility_dependencies(
    index: u32,
    snapshot: &LoadoutFacilityPlacementSnapshot,
) -> TestResult<BTreeSet<u32>> {
    let mut required = BTreeSet::from([index]);
    required.extend(
        snapshot
            .configuration_dependencies
            .iter()
            .filter_map(|dependency| {
                matches!(dependency.state, AlignmentDependencyState::Complete)
                    .then_some(dependency.record_index)
            }),
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
    for row in &snapshot.rows {
        for (_, owner) in row.configurations() {
            required.insert(
                *owner
                    .record_index()
                    .value()
                    .ok_or("the physical configuration owner is absent")?,
            );
        }
        if let Some(port) = row.original_port() {
            required.insert(port.owner_record_index());
        }
    }
    Ok(required)
}

fn produce(archive: &Archive, input: &DataForgeEntry, filter: &Filter) -> TestResult<BuiltCatalog> {
    let options = CatalogOptions {
        filter: Some(filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    build_catalog_from(archive, input, &options)
        .map_err(|_| "build the exact normal Catalog scope".into())
}

fn document(archive: &Archive, output: &BuiltCatalog) -> TestResult<Value> {
    let document: Value =
        serde_json::from_slice(output.json()).map_err(|_| "read the scoped Catalog JSON")?;
    verify_build(archive, &document)?;
    if document.get("schemaVersion").and_then(Value::as_u64) != Some(u64::from(SCHEMA_VERSION))
        || document
            .pointer("/exportHealth/publicationMode")
            .and_then(Value::as_str)
            != Some("normal")
    {
        return Err("the scoped producer lacks the current normal Catalog wire contract".into());
    }
    Ok(document)
}

fn ship_row(document: &Value, index: u32) -> TestResult<&Value> {
    let mut selected = document
        .pointer("/ships")
        .and_then(Value::as_array)
        .ok_or("the normal producer omitted the Ship table")?
        .iter()
        .filter(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(index))
        });
    let row = selected
        .next()
        .ok_or("the scoped producer omitted the physical Ship")?;
    if selected.next().is_some() {
        return Err("the scoped producer duplicated the physical Ship Record".into());
    }
    Ok(row)
}

const fn complete(coverage: &CatalogRecordCoverage) -> bool {
    coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
}
