// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;

use p4kx_export::catalog_queries::shops::{
    Shop, ShopRegistrySelection, ShopRowSnapshot, ShopRowVisitSummary,
    read_shop_with_provenance_mode, visit_shops_with_provenance_mode,
};
use p4kx_export::catalog_queries::{DatasetAvailability, ProvenanceMode};
use p4kx_export::{CatalogError, StarmapTraversalBudget};
use p4kx_extraction::{Archive, PhysicalSourceKind, SourceInventory, SourceLocation, SourcedValue};

use serde_json::Value;

use super::equipment_input::with_archive;
use super::report_scoped_rows;
use crate::real_archive_catalog_table_parity::{
    Origins, TestResult, check_selected_row, wire_row, with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;
use crate::real_archive_weapon_combat::ammo_container_replay;

#[path = "real_archive_shop_original_nomination.rs"]
mod original_nomination;

const REGISTRY: &str = r"Data\Libs\CosmoTools\LayerRecovery\LayerBackups.xml";
const ENTRY: &str = "P4KX_SHOP_SDK_REGISTRY_ENTRY";
const NODE: &str = "P4KX_SHOP_SDK_REGISTRY_NODE";

#[test]
#[ignore = "real Archive: use the exact Build and one located physical Shop in its own measured process"]
fn installed_physical_shop_located_cold_query() -> TestResult {
    with_archive(|archive| {
        let node = coordinate(NODE)?;
        let snapshot = read_registry_node(archive, node)?;
        let shop = snapshot
            .row
            .as_ref()
            .ok_or("the nominated Shop was not admitted")?;
        check_located(&snapshot, shop)?;
        check_key_source(&snapshot, shop)?;
        let rows = [wire_row(shop)?];
        Origins::default().extend(&snapshot.source_inventory, &rows)?;
        report_scoped_rows("Physical Shop", "located-cold", &rows)
    })
}

#[test]
#[ignore = "same Build: compare an admitted physical Shop with a retained Normal Catalog"]
fn installed_physical_shop_matches_retained_complete_catalog() -> TestResult {
    verify_retained_catalog(Replay::Shop)
}

#[test]
#[ignore = "same Build: compare one Shop and two original magazine slots in one retained Normal stream"]
fn installed_physical_shop_and_ammo_containers_match_retained_complete_catalog() -> TestResult {
    verify_retained_catalog(Replay::ShopAndAmmo)
}

#[test]
#[ignore = "real Archive: run the complete Shop SDK visit once per Build in a measured process"]
fn installed_all_shops_cold_query() -> TestResult {
    with_archive(|archive| {
        let summary = visit_shops_with_provenance_mode::<CatalogError>(
            archive,
            StarmapTraversalBudget::default(),
            ProvenanceMode::Full,
            None,
            |_| Ok(()),
        )?;
        validate_all_shop_summary(archive, &summary)?;
        print_line(&format!(
            "complete Shop SDK cold visit: rows={}",
            summary.row_count
        ));
        Ok(())
    })
}

#[test]
#[ignore = "same Build: compare all ordered Shops and original field evidence with a retained Normal Catalog"]
fn installed_all_shops_match_retained_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_SHOP_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Shop Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let mut rows = Vec::new();
        let summary = visit_shops_with_provenance_mode::<CatalogError>(
            archive,
            StarmapTraversalBudget::default(),
            ProvenanceMode::Full,
            None,
            |shop| {
                rows.push(serde_json::to_value(shop).map_err(CatalogError::Json)?);
                Ok(())
            },
        )?;
        validate_all_shop_summary(archive, &summary)?;
        if rows.len() != summary.row_count {
            return Err("the complete Shop visit count differs from its output".into());
        }
        let mut origins = Origins::default();
        origins.extend(&summary.source_inventory, &rows)?;
        measurement.verify_tables_and_arrays([], &[], &[("/shops", &rows)], &origins)?;
        report_scoped_rows("Physical Shop", "all-retained-parity", &rows)
    })
}

fn validate_all_shop_summary(archive: &Archive, summary: &ShopRowVisitSummary) -> TestResult {
    if archive.build().is_none()
        || summary.build.as_ref() != archive.build()
        || !matches!(summary.availability, DatasetAvailability::Available)
        || summary.row_count == 0
        || summary.coverage.names.is_none()
    {
        return Err("expected actual located Shop rows from the exact measured Build".into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Replay {
    Shop,
    ShopAndAmmo,
}

fn verify_retained_catalog(replay: Replay) -> TestResult {
    let revision = env::var("P4KX_SHOP_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Shop Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let snapshot = read_registry_node(archive, coordinate(NODE)?)?;
        let shop = snapshot
            .row
            .as_ref()
            .ok_or("the nominated Shop was not admitted")?;
        check_located(&snapshot, shop)?;
        check_key_source(&snapshot, shop)?;
        let key = shop
            .registry
            .key
            .value()
            .ok_or("expected the original Shop key")?;
        let row = wire_row(shop)?;
        let rows = std::slice::from_ref(&row);
        let mut origins = Origins::default();
        origins.extend(&snapshot.source_inventory, rows)?;
        let same_shop = |candidate: &Value| {
            candidate.pointer("/key/value").and_then(Value::as_str) == Some(key.as_str())
        };
        match replay {
            Replay::Shop => measurement.verify_selected_row("/shops", &row, &origins, same_shop)?,
            Replay::ShopAndAmmo => {
                let ammo = ammo_container_replay::read(archive, &mut origins)?;
                let [containers, bindings] = ammo_container_replay::PATHS;
                measurement.verify_selected_scope(
                    &["/shops", containers, bindings],
                    &origins,
                    |catalog| {
                        check_selected_row(catalog, "/shops", &row, same_shop)?;
                        ammo_container_replay::verify(catalog, &ammo)
                    },
                )?;
                print_line(
                    "Ammo container retained parity: two physical slots and two original bindings",
                );
            }
        }
        report_scoped_rows("Physical Shop", "retained-parity", rows)
    })
}

#[test]
#[ignore = "real Archive: use the same Build and registry Entry as the located Shop proof"]
fn installed_physical_shop_absent_node() -> TestResult {
    with_archive(|archive| {
        let snapshot = read_registry_node(archive, u32::MAX)?;
        if snapshot.row.is_some()
            || snapshot.coverage.declarations_complete.is_some()
            || snapshot.coverage.inventory_payloads != 0
            || snapshot.coverage.names.is_some()
        {
            return Err(
                "the unadmitted node must remain a closed negative without Shop joins".into(),
            );
        }
        report_scoped_rows("Physical Shop", "absent-node", &[])
    })
}

fn coordinate(var: &str) -> TestResult<u32> {
    env::var(var)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "configure the original physical Shop registry coordinate".into())
}

fn read_registry_node(archive: &Archive, node: u32) -> TestResult<ShopRowSnapshot> {
    let entry_index = coordinate(ENTRY)?;
    let entry = archive
        .entries()
        .get(usize::try_from(entry_index)?)
        .ok_or("the nominated registry Entry is not in this Build")?;
    if u32::try_from(entry.index())? != entry_index || !entry.path().eq_ignore_ascii_case(REGISTRY)
    {
        return Err("the nominated Entry must be the original Shop registry".into());
    }
    let selection = ShopRegistrySelection::new(entry_index, node);
    let snapshot = read_shop_with_provenance_mode(
        archive,
        selection,
        StarmapTraversalBudget::default(),
        ProvenanceMode::Full,
        None,
    )
    .map_err(|_| "read the selected physical Shop from the installed Archive")?;
    if snapshot.build.as_ref() != archive.build()
        || snapshot.selection != selection
        || !matches!(snapshot.availability, DatasetAvailability::Available)
        || !snapshot.coverage.registry_complete
    {
        return Err(
            "the original registry and requested physical identity must be conclusive".into(),
        );
    }
    Ok(snapshot)
}

fn check_located(snapshot: &ShopRowSnapshot, shop: &Shop) -> TestResult {
    let names = snapshot
        .coverage
        .names
        .as_ref()
        .ok_or("nominate a Shop with a registry location")?;
    if !names.routes_complete
        || !names.records.unreadable_headers.is_empty()
        || !names.records.unreadable_bodies.is_empty()
        || !names.records.incomplete_fields.is_empty()
        || shop.starmap_object_guid.value().is_none()
        || !shop.starmap_object_guid.origin().source_set_complete()
    {
        return Err("the nominated Shop must have a conclusive, sourced Starmap link".into());
    }
    if snapshot.coverage.declarations_complete != Some(true)
        || snapshot.coverage.inventory_payloads != 1
        || shop
            .item_guids
            .value()
            .is_none_or(|items| items.as_slice().is_empty())
    {
        return Err("the selected Shop must reach one original inventory with item values".into());
    }
    Ok(())
}

fn check_key_source(snapshot: &ShopRowSnapshot, shop: &Shop) -> TestResult {
    let selection = registry_coordinate(&shop.registry.key, &snapshot.source_inventory)?;
    if selection != snapshot.selection {
        return Err("the Shop key refers to a different original registry node".into());
    }
    Ok(())
}

fn registry_coordinate(
    key: &SourcedValue<String>,
    inventory: &SourceInventory,
) -> TestResult<ShopRegistrySelection> {
    let origin = key.origin();
    if key.value().is_none() || !origin.source_set_complete() {
        return Err("the Shop key must retain its complete original registry source".into());
    }
    let mut coordinate = None;
    for usage in inventory.iter_uses() {
        if !origin.source_uses_refs().contains(usage.id()) {
            continue;
        }
        let SourceLocation::Subresource {
            namespace,
            indices,
            field,
        } = usage.location()
        else {
            continue;
        };
        if namespace != "xml" || field != "@ID" {
            continue;
        }
        let node = indices
            .first()
            .copied()
            .ok_or("missing original @ID node index")?;
        let Some(source) = inventory
            .iter_sources()
            .find(|source| source.id() == usage.source())
        else {
            return Err("the original @ID source is missing".into());
        };
        let PhysicalSourceKind::ArchiveEntry(entry) = source.kind() else {
            continue;
        };
        if entry
            .path()
            .value()
            .is_none_or(|path| !path.eq_ignore_ascii_case(REGISTRY))
        {
            continue;
        }
        let selection =
            ShopRegistrySelection::new(u32::try_from(entry.entry_index())?, u32::try_from(node)?);
        if coordinate
            .replace(selection)
            .is_some_and(|previous| previous != selection)
        {
            return Err("the Shop key refers to more than one original registry node".into());
        }
    }
    coordinate.ok_or_else(|| "the Shop key lacks its original registry Entry and @ID node".into())
}
