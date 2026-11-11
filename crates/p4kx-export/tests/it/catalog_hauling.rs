// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Hauling orders of contracts and cargo manifests in the Basetool catalog.

use crate::hauling_fixture::{
    BROKERED, CRATE, CRATES, FAR, GOLD, MANIFEST_CUSTOM, MANIFEST_EVEN, NAMELESS, NEAR, NOTHING,
    PLAIN, SCRAP, SUB, guid, write_archive,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build() -> Result<(p4kx_export::BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn orders(
    catalog: &Value,
    contract: u128,
    sub: Option<u128>,
) -> Result<&Vec<Value>, Box<dyn Error>> {
    let contract = guid(contract);
    let sub = sub.map(guid);
    crate::catalog_contracts::entry(catalog, &contract, sub.as_deref())?["haulingOrders"]
        .as_array()
        .ok_or_else(|| format!("no hauling orders of {contract}").into())
}

fn resource(guid_value: u128, min: i64, max: i64, container: i64) -> Value {
    json!({
        "kind": "resource", "resourceGuid": guid(guid_value), "entityClassGuid": null,
        "entityClassesGuid": null, "minScu": min, "maxScu": max, "maxContainerSize": container,
        "minAmount": null, "maxAmount": null
    })
}

fn pieces(kind: &str, field: &str, target: u128, min: i64, max: i64) -> Value {
    let mut item = json!({
        "kind": kind, "resourceGuid": null, "entityClassGuid": null, "entityClassesGuid": null,
        "minScu": null, "maxScu": null, "maxContainerSize": null,
        "minAmount": min, "maxAmount": max
    });
    if let Some(object) = item.as_object_mut() {
        object.insert(field.to_owned(), guid(target).into());
    }
    item
}

fn mission_item(min: i64, max: i64) -> Value {
    json!({
        "kind": "missionItem", "resourceGuid": null, "entityClassGuid": null,
        "entityClassesGuid": null, "minScu": null, "maxScu": null, "maxContainerSize": null,
        "minAmount": min, "maxAmount": max
    })
}

fn single(item: Value) -> Value {
    let mut requirement = serde_json::Map::new();
    requirement.insert(
        "options".to_owned(),
        Value::Array(vec![Value::Array(vec![item])]),
    );
    Value::Object(requirement)
}

#[test]
fn a_contract_lists_the_nearest_property_per_variable_and_the_template_objective() -> TestResult {
    let (_, catalog) = build()?;
    let handler_only = json!({
        "source": "handler", "variableName": "HandlerOnly",
        "requirements": [single(resource(GOLD, 2, 2, -1))]
    });
    assert_eq!(
        *orders(&catalog, NEAR, None)?,
        vec![
            json!({
                "source": "contract", "variableName": "HaulingOverride",
                "requirements": [
                    single(resource(SCRAP, 4, 8, 16)),
                    single(pieces("entityClass", "entityClassGuid", CRATE, 3, 0)),
                    single(pieces("entityClasses", "entityClassesGuid", CRATES, 1, 4)),
                    single(mission_item(1, 0)),
                ]
            }),
            handler_only,
            json!({
                "source": "templateObjective", "variableName": null,
                "requirements": [
                    single(resource(SCRAP, 1, 0, -1)),
                    single(pieces("entityClass", "entityClassGuid", CRATE, 12, 12)),
                ]
            }),
        ]
    );
    Ok(())
}

#[test]
fn a_sub_contract_overrides_the_contract_and_an_or_lists_its_options() -> TestResult {
    let (_, catalog) = build()?;
    let sub = orders(&catalog, NEAR, Some(SUB))?;
    assert_eq!(sub.len(), 3);
    assert_eq!(sub[0]["source"], "subContract");
    assert_eq!(sub[0]["variableName"], "HaulingOverride");
    assert_eq!(
        sub[0]["requirements"],
        json!([{"options": [
            [resource(SCRAP, 10, 10, -1)],
            [
                resource(GOLD, 5, 5, -1),
                pieces("entityClass", "entityClassGuid", CRATE, 2, 0),
            ],
        ]}])
    );
    assert_eq!(sub[1]["source"], "handler");
    Ok(())
}

#[test]
fn the_template_property_is_the_last_level_and_the_handler_wins_over_it() -> TestResult {
    let (_, catalog) = build()?;
    let far = orders(&catalog, FAR, None)?;
    let sources: Vec<_> = far
        .iter()
        .map(|order| (order["source"].as_str(), order["variableName"].as_str()))
        .collect();
    assert_eq!(
        sources,
        [
            (Some("handler"), Some("HaulingOverride")),
            (Some("handler"), Some("HandlerOnly")),
            (Some("templateObjective"), None),
        ]
    );
    assert_eq!(
        far[0]["requirements"],
        json!([single(resource(GOLD, 1, 1, -1))])
    );
    Ok(())
}

#[test]
fn a_broker_entry_states_the_objective_orders_of_a_contract_without_template() -> TestResult {
    let (_, catalog) = build()?;
    let brokered = orders(&catalog, BROKERED, None)?;
    assert_eq!(brokered.len(), 3);
    assert_eq!(
        brokered[2],
        json!({
            "source": "brokerObjective", "variableName": null,
            "requirements": [single(resource(GOLD, 2, 6, 32))]
        })
    );
    let plain = orders(&catalog, PLAIN, None)?;
    assert!(plain.iter().all(|order| order["source"] == "handler"));
    Ok(())
}

#[test]
fn properties_without_a_variable_name_shadow_nothing() -> TestResult {
    let (_, catalog) = build()?;
    let unnamed = orders(&catalog, NAMELESS, None)?;
    let found: Vec<_> = unnamed
        .iter()
        .map(|order| (order["source"].as_str(), order["variableName"].clone()))
        .collect();
    assert_eq!(
        found,
        [
            (Some("contract"), Value::Null),
            (Some("handler"), Value::Null)
        ]
    );
    assert_eq!(
        unnamed[0]["requirements"],
        json!([single(resource(GOLD, 4, 4, -1))])
    );
    assert_eq!(
        unnamed[1]["requirements"],
        json!([single(resource(SCRAP, 3, 3, -1))])
    );
    Ok(())
}

#[test]
fn a_contract_that_hauls_nothing_has_no_orders() -> TestResult {
    let (_, catalog) = build()?;
    assert_eq!(*orders(&catalog, NOTHING, None)?, Vec::<Value>::new());
    Ok(())
}

#[test]
fn cargo_manifests_list_their_fill_resources_and_spawn_rules() -> TestResult {
    let (built, catalog) = build()?;
    let manifests = catalog["cargoManifests"]
        .as_array()
        .ok_or("cargoManifests")?;
    assert_eq!(manifests.len(), 2);
    let even = manifests
        .iter()
        .find(|entry| entry["guid"] == guid(MANIFEST_EVEN).as_str())
        .ok_or("even manifest")?;
    assert_eq!(even["className"], "LegalCargo_Even");
    assert_eq!(even["fillType"], "even");
    assert_eq!(even["minCapacityRange"], json!(0.45));
    assert_eq!(even["maxCapacityRange"], json!(0.5));
    assert_eq!(even["lockCargoGridsOnSpawn"], false);
    assert_eq!(
        even["resources"],
        json!([
            {"resourceGuid": guid(SCRAP), "probability": 1.0, "minAllocation": null, "maxAllocation": null},
            {"resourceGuid": guid(GOLD), "probability": 0.25, "minAllocation": null, "maxAllocation": null},
        ])
    );
    assert_eq!(
        even["spawnRules"],
        json!([
            {
                "kind": "entities", "probability": 1.0,
                "entityGuids": [guid(CRATE), guid(CRATES)], "resourceGuids": [],
                "quantityAlgorithm": "CargoManifestFixedQuantityDef",
                "quantity": 30, "proportion": null
            },
            {
                "kind": "resources", "probability": 1.0,
                "entityGuids": [], "resourceGuids": [guid(CRATE), guid(CRATES)],
                "quantityAlgorithm": "CargoManifestProportionQuantityDef",
                "quantity": null, "proportion": 0.5
            },
        ])
    );
    let custom = manifests
        .iter()
        .find(|entry| entry["guid"] == guid(MANIFEST_CUSTOM).as_str())
        .ok_or("custom manifest")?;
    assert_eq!(custom["fillType"], "custom");
    assert_eq!(custom["minCapacityRange"], Value::Null);
    assert_eq!(
        custom["resources"],
        json!([{
            "resourceGuid": guid(GOLD), "probability": 1.0,
            "minAllocation": 0.03, "maxAllocation": 0.06
        }])
    );
    assert_eq!(custom["spawnRules"], json!([]));
    let kind = built
        .summary()
        .kind(Kind::CargoManifests)
        .ok_or("summary")?;
    assert_eq!((kind.count, kind.skipped), (2, 0));
    Ok(())
}
