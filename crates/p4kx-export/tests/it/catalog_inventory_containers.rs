// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_inventory_container_fixture.rs"]
mod fixture;

#[test]
fn inventory_records_keep_original_open_restrictions_and_closed_units() -> TestResult {
    let database = fixture::database();
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_directory, archive) = common::open(&database, origin)?;
        let catalog: Value = serde_json::from_slice(
            p4kx_export::build_catalog(&archive, &common::options())?.json(),
        )?;
        let rows = proof::rows(&catalog, "/definitions/inventoryContainers/rows")?;
        assert_eq!(
            rows.len(),
            5,
            "one definition for each physical source Record"
        );
        validator
            .validate(&catalog)
            .map_err(|error| format!("schema: {error}"))?;
        check_open(row(rows, fixture::OPEN)?, encrypted)?;
        check_closed(rows, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn row(rows: &[Value], guid: u128) -> TestResult<&Value> {
    let guid = common::guid(guid);
    rows.iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(&guid))
        .ok_or_else(|| "original InventoryContainer definition".into())
}

fn check_open(row: &Value, encrypted: bool) -> TestResult {
    proof::present(row, "recordIndex", &json!(0), Some(encrypted))?;
    proof::present(
        row,
        "sourceType",
        &json!("InventoryContainer"),
        Some(encrypted),
    )?;
    let interior = proof::at(row, "/interiorDimensions/value")?;
    single(interior, "x", 0.0, encrypted)?;
    single(interior, "y", -0.0, encrypted)?;
    single(interior, "z", 4.25, encrypted)?;
    let container = proof::at(row, "/inventoryType/value")?;
    assert_eq!(
        container["kind"], "open",
        "authored concrete inventory type"
    );
    proof::present(
        container,
        "isExternalContainer",
        &json!(false),
        Some(encrypted),
    )?;
    let grid = proof::at(container, "/gridCellSize/value")?;
    proof::present(grid, "centimeters", &json!(0), Some(encrypted))?;
    let minimum = proof::at(container, "/minPermittedItemSize/value")?;
    single(minimum, "x", 0.0, encrypted)?;
    let maximum = proof::at(container, "/maxPermittedItemSize/value")?;
    single(maximum, "z", 5.0, encrypted)?;
    let offset = proof::at(container, "/gridPosOffset/value")?;
    single(offset, "x", -0.0, encrypted)?;
    check_restrictions(row, container, encrypted)?;
    proof::present(
        container,
        "maxPercentageErasedOnParentDestruction",
        &json!(0),
        Some(encrypted),
    )?;
    single(
        container,
        "randomDestructionDistributionExponent",
        1.25,
        encrypted,
    )?;
    single(
        container,
        "appliedForceOnParentDestruction",
        -0.0,
        encrypted,
    )?;
    Ok(())
}

fn check_restrictions(row: &Value, container: &Value, encrypted: bool) -> TestResult {
    let restrictions = proof::rows(container, "/overrideStorableItemTypes/value")?;
    assert_eq!(restrictions.len(), 2, "repeated original filters survive");
    let restriction = proof::at(row, "/excludedItemSubTypes/value/0/value")?;
    proof::present(
        restriction,
        "itemType",
        &json!("FixtureUtility"),
        Some(encrypted),
    )?;
    let subtypes = proof::rows(restriction, "/itemSubTypes/value")?;
    let expected = ["FixtureSmall", "FixtureMedium", "FixtureSmall"];
    assert_eq!(subtypes.len(), expected.len(), "every stored subtype slot");
    for (slot, label) in subtypes.iter().zip(expected) {
        assert_eq!(slot["state"], "present", "a readable subtype enum slot");
        assert_eq!(slot["value"], label, "ordered and repeated enum labels");
    }
    Ok(())
}

fn check_closed(rows: &[Value], encrypted: bool) -> TestResult {
    let capacity = |guid| proof::at(row(rows, guid)?, "/inventoryType/value/capacity/value");
    single(
        capacity(fixture::STANDARD)?,
        "standardCargoUnits",
        2.75,
        encrypted,
    )?;
    proof::present(
        capacity(fixture::CENTI)?,
        "centiSCU",
        &json!(-125),
        Some(encrypted),
    )?;
    proof::present(
        capacity(fixture::MICRO)?,
        "microSCU",
        &json!(0),
        Some(encrypted),
    )?;
    let future = capacity(fixture::FUTURE)?;
    assert_eq!(
        future["kind"], "unknownUnit",
        "the original unit is classified"
    );
    let original = proof::at(future, "/additionalFields/roots/0")?;
    proof::present(original, "name", &json!("fixtureQuanta"), Some(encrypted))?;
    Ok(())
}

fn single(row: &Value, field: &str, original: f32, encrypted: bool) -> TestResult {
    let expected = json!({"rawBits": format!("{:08x}", original.to_bits()), "finite": original});
    proof::present(row, field, &expected, Some(encrypted))?;
    Ok(())
}

#[path = "catalog_inventory_entities_fixture.rs"]
mod entities;

#[test]
fn inventory_owner_rows_keep_references_bounds_and_storage_policies_separate() -> TestResult {
    let (_directory, archive) = common::open(&entities::database(), DcbOrigin::Encrypted)?;
    let options = p4kx_export::CatalogOptions {
        publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(p4kx_export::build_catalog(&archive, &options)?.json())?;
    check_storage_owners(&catalog)?;
    check_bounds(&catalog)?;
    jsonschema::validator_for(&p4kx_export::catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("schema: {error}"))?;
    proof::closure(&catalog)?;
    Ok(())
}

fn check_storage_owners(catalog: &Value) -> TestResult {
    let storage = proof::rows(catalog, "/configurations/inventoryConfigurations/rows")?;
    assert_eq!(storage.len(), 3, "physical storage component slots");
    let item = component(storage, entities::CARGO_ITEM, 0)?;
    let duplicate = component(storage, entities::CARGO_ITEM, 1)?;
    let vehicle = component(storage, entities::VEHICLE, 0)?;
    assert_eq!(item["kind"], "itemStorage", "original item component kind");
    assert_eq!(
        vehicle["kind"], "vehicleStorage",
        "original vehicle component kind"
    );
    check_selector(item, "containerParams", fixture::OPEN, "present")?;
    check_selector(duplicate, "containerParams", entities::MISSING, "dangling")?;
    check_selector(
        vehicle,
        "inventoryContainerParams",
        fixture::STANDARD,
        "present",
    )?;
    proof::present(
        item,
        "canAutoStoreRestrictedItems",
        &json!(false),
        Some(true),
    )?;
    proof::cell(item, "openInventoryInteraction", "present")?;
    proof::cell(item, "openLootingInteraction", "null")?;
    Ok(())
}

fn check_bounds(catalog: &Value) -> TestResult {
    let bounds = proof::rows(catalog, "/configurations/itemBounds/rows")?;
    assert_eq!(
        bounds.len(),
        4,
        "separate original coordinates and authored metadata"
    );
    check_occupancy(component(bounds, entities::WEARABLE, 0)?)?;
    let vehicle_bounds = proof::at(
        component(bounds, entities::VEHICLE, 0)?,
        "/maxBoundingBoxSize/value",
    )?;
    single(vehicle_bounds, "z", 7.0, true)?;
    proof::present(
        component(bounds, entities::WEARABLE, 1)?,
        "hasLocalBounds",
        &json!(false),
        Some(true),
    )?;
    proof::present(
        component(bounds, entities::VEHICLE, 1)?,
        "hasLocalBounds",
        &json!(true),
        Some(true),
    )?;
    Ok(())
}

fn check_occupancy(wearable: &Value) -> TestResult {
    let definition = proof::at(wearable, "/attachDef/value")?;
    let volume = proof::at(definition, "/inventoryOccupancyVolume/value")?;
    proof::present(volume, "microSCU", &json!(0), Some(true))?;
    let dimensions = proof::at(definition, "/inventoryOccupancyDimensions/value")?;
    single(dimensions, "x", 0.0, true)?;
    single(dimensions, "y", -0.0, true)?;
    proof::present(
        dimensions,
        "z",
        &json!({"rawBits": format!("{:08x}", entities::DIMENSION_BITS), "finite": null}),
        Some(true),
    )?;
    proof::cell(definition, "inventoryOccupancyDimensionsUIOverride", "null")?;
    let minimum = proof::at(definition, "/inventoryOccupancyLocalBoundsMin/value")?;
    single(minimum, "x", -1.0, true)?;
    let maximum = proof::at(definition, "/inventoryOccupancyLocalBoundsMax/value")?;
    single(maximum, "z", 3.0, true)?;
    let fixed = proof::at(
        definition,
        "/inventoryOccupancyFixedGridDimensionsUIV4/value",
    )?;
    single(fixed, "y", 0.0, true)?;
    Ok(())
}

fn component(rows: &[Value], guid: u128, slot: u32) -> TestResult<&Value> {
    let guid = common::guid(guid);
    rows.iter()
        .find(|row| {
            row.pointer("/ownerGuid/value").and_then(Value::as_str) == Some(&guid)
                && row
                    .pointer("/componentOrdinal/value")
                    .and_then(Value::as_u64)
                    == Some(u64::from(slot))
        })
        .ok_or_else(|| format!("storage fixture owner {guid} slot {slot}").into())
}

fn check_selector(row: &Value, field: &str, target: u128, state: &str) -> TestResult {
    let link = proof::at(row, &format!("/{field}/value"))?;
    assert_eq!(
        link["status"], state,
        "original storage Reference resolution"
    );
    proof::present(link, "guid", &json!(common::guid(target)), Some(true))?;
    let index = if state == "present" {
        "present"
    } else {
        "absent"
    };
    proof::cell(link, "targetRecordIndex", index)?;
    Ok(())
}
