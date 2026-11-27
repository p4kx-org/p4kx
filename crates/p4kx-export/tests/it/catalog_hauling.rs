// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Hauling orders of contracts and cargo manifests in the P4KX catalog.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::hauling_fixture::{
    BROKERED, CRATE, CRATES, FAR, GOLD, NAMELESS, NEAR, NOTHING, PLAIN, SCRAP, SUB, guid,
    write_archive,
};

#[path = "catalog_hauling_manifests.rs"]
mod manifests;

type TestResult = Result<(), Box<dyn Error>>;

fn build() -> Result<(p4kx_export::BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn json_at<'row>(value: &'row Value, pointer: &str) -> Result<&'row Value, Box<dyn Error>> {
    value
        .pointer(pointer)
        .ok_or_else(|| format!("missing JSON field {pointer}").into())
}

fn json_array<'row>(value: &'row Value, pointer: &str) -> Result<&'row [Value], Box<dyn Error>> {
    json_at(value, pointer)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected JSON array at {pointer}").into())
}

fn orders(catalog: &Value, contract: u128, sub: Option<u128>) -> Result<&[Value], Box<dyn Error>> {
    let contract = guid(contract);
    let sub = sub.map(guid);
    let entry = crate::catalog_contracts::entry(catalog, &contract, sub.as_deref())?;
    json_array(entry, "/haulingOrders")
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
        orders(&catalog, NEAR, None)?,
        [
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
        ],
        "nearer definition wins for each authored variable and keeps the objective"
    );
    Ok(())
}

#[test]
fn a_sub_contract_overrides_the_contract_and_an_or_lists_its_options() -> TestResult {
    let (_, catalog) = build()?;
    let [overridden, handler, _] = orders(&catalog, NEAR, Some(SUB))? else {
        return Err("three sub-contract hauling orders".into());
    };
    assert_eq!(
        json_at(overridden, "/source")?,
        "subContract",
        "override source"
    );
    assert_eq!(
        json_at(overridden, "/variableName")?,
        "HaulingOverride",
        "overridden variable"
    );
    assert_eq!(
        json_at(overridden, "/requirements")?,
        &json!([{"options": [
            [resource(SCRAP, 10, 10, -1)],
            [
                resource(GOLD, 5, 5, -1),
                pieces("entityClass", "entityClassGuid", CRATE, 2, 0),
            ],
        ]}]),
        "OR retains both authored alternatives"
    );
    assert_eq!(
        json_at(handler, "/source")?,
        "handler",
        "handler contributes its unrelated variable"
    );
    Ok(())
}

#[test]
fn the_template_property_is_the_last_level_and_the_handler_wins_over_it() -> TestResult {
    let (_, catalog) = build()?;
    let far = orders(&catalog, FAR, None)?;
    let sources: Vec<_> = far
        .iter()
        .map(|order| {
            (
                order.get("source").and_then(Value::as_str),
                order.get("variableName").and_then(Value::as_str),
            )
        })
        .collect();
    assert_eq!(
        sources,
        [
            (Some("handler"), Some("HaulingOverride")),
            (Some("handler"), Some("HandlerOnly")),
            (Some("templateObjective"), None),
        ],
        "handler wins over the same template variable and the objective remains separate"
    );
    let first = far.first().ok_or("overriding handler order")?;
    assert_eq!(
        json_at(first, "/requirements")?,
        &json!([single(resource(GOLD, 1, 1, -1))]),
        "handler order retains its resource requirement"
    );
    Ok(())
}

#[test]
fn a_broker_entry_states_the_objective_orders_of_a_contract_without_template() -> TestResult {
    let (_, catalog) = build()?;
    let [_, _, objective] = orders(&catalog, BROKERED, None)? else {
        return Err("three hauling orders from the brokered contract".into());
    };
    assert_eq!(
        objective,
        &json!({
            "source": "brokerObjective", "variableName": null,
            "requirements": [single(resource(GOLD, 2, 6, 32))]
        }),
        "the broker provides the objective when the contract has no template"
    );
    let plain = orders(&catalog, PLAIN, None)?;
    assert!(
        plain
            .iter()
            .all(|order| order.get("source").and_then(Value::as_str) == Some("handler")),
        "plain contract has only handler orders"
    );
    Ok(())
}

#[test]
fn properties_without_a_variable_name_shadow_nothing() -> TestResult {
    let (_, catalog) = build()?;
    let [contract, handler] = orders(&catalog, NAMELESS, None)? else {
        return Err("two independently retained nameless hauling properties".into());
    };
    for (order, source, item) in [
        (contract, "contract", resource(GOLD, 4, 4, -1)),
        (handler, "handler", resource(SCRAP, 3, 3, -1)),
    ] {
        assert_eq!(
            json_at(order, "/source")?,
            source,
            "nameless property keeps its source"
        );
        assert!(
            json_at(order, "/variableName")?.is_null(),
            "nameless property has an explicit null name"
        );
        assert_eq!(
            json_at(order, "/requirements")?,
            &json!([single(item)]),
            "nameless {source} property's resource requirement"
        );
    }
    Ok(())
}

#[test]
fn a_contract_that_hauls_nothing_has_no_orders() -> TestResult {
    let (_, catalog) = build()?;
    assert!(
        orders(&catalog, NOTHING, None)?.is_empty(),
        "contract has no authored hauling requirements"
    );
    Ok(())
}

#[test]
fn cargo_manifests_list_their_fill_resources_and_spawn_rules() -> TestResult {
    manifests::verify()
}
