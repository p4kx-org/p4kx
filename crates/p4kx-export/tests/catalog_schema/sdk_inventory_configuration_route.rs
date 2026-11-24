// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{
    InventoryConfigurationSnapshot, with_inventory_configurations,
    with_inventory_configurations_from,
};
use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use super::super::super::INVENTORY_CONFIGURATIONS;
use super::{DataForgeStorage, Route, TestResult, entities, open};

pub(super) fn read() -> TestResult<Route> {
    let (_folder, archive) = open(&entities::database(), DataForgeStorage::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let catalog: Value = serde_json::from_slice(
        build_catalog_from(&archive, &entry, &CatalogOptions::new(UNIX_EPOCH))?.json(),
    )?;
    let (route, vehicle_record) = with_inventory_configurations_from(
        &archive,
        &entry,
        CatalogRecordSelection::All,
        None,
        |snapshot| check(&snapshot, &catalog),
    )??;
    with_inventory_configurations_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(vehicle_record),
        None,
        |snapshot| -> TestResult {
            let [vehicle] = snapshot.rows() else {
                return Err(
                    "only the requested physical Vehicle storage row should project".into(),
                );
            };
            assert_eq!(
                Some(&serde_json::to_value(vehicle)?),
                catalog.pointer("/configurations/inventoryConfigurations/rows/2"),
                "physical selection retains the exact canonical Vehicle storage row"
            );
            Ok(())
        },
    )??;
    check_unrelated_bounds()?;
    Ok(route)
}

fn check(
    snapshot: &InventoryConfigurationSnapshot<'_>,
    catalog: &Value,
) -> TestResult<(Route, u32)> {
    let [item, repeated, vehicle] = snapshot.rows() else {
        return Err("the fixture has two Item components and one Vehicle component".into());
    };
    let actual = json!({"availability": snapshot.availability(), "rows": snapshot.rows()});
    assert_eq!(
        Some(&actual),
        catalog.pointer(INVENTORY_CONFIGURATIONS),
        "the selective query retains the canonical ordered storage rows and their origins"
    );
    assert_eq!(
        item.owner().record_index().value(),
        repeated.owner().record_index().value(),
        "both original Item storage components belong to the same physical owner"
    );
    for row in [item, repeated] {
        assert!(
            row.item_container_params().is_some() && row.vehicle_container_params().is_none(),
            "each Item row retains its typed original container Reference"
        );
    }
    assert!(
        vehicle.item_container_params().is_none() && vehicle.vehicle_container_params().is_some(),
        "the Vehicle component supplies its own typed inventoryContainerParams Reference"
    );
    for row in snapshot.rows() {
        assert_eq!(
            row.source_type().origin().contains_encrypted_source(),
            Some(true),
            "each storage row retains the encrypted original source"
        );
    }
    assert_eq!(
        snapshot.coverage().incomplete_fields,
        [] as [u32; 0],
        "the complete storage fixture has no incomplete fields"
    );
    let route = Route::queried_rows(INVENTORY_CONFIGURATIONS, snapshot.rows(), None)?;
    let record = vehicle
        .owner()
        .record_index()
        .value()
        .copied()
        .ok_or("physical Vehicle owner")?;
    Ok((route, record))
}

fn check_unrelated_bounds() -> TestResult {
    let (_folder, archive) = open(&entities::wrong_dimension_type(), DataForgeStorage::Plain)?;
    with_inventory_configurations(
        &archive,
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            let [] = snapshot.rows() else {
                return Err(
                    "an unrelated Item Bounds fixture must not project storage rows".into(),
                );
            };
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [] as [u32; 0],
                "an unrelated malformed bounds field does not affect storage coverage"
            );
            Ok(())
        },
    )?
}
