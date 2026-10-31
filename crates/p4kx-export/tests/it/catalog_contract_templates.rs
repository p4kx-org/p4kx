// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Contract templates in the Basetool catalog.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::spawns_fixture;

type TestResult = Result<(), Box<dyn Error>>;

const TEMPLATE_GUID: &str = "40000000-0000-4000-8000-000000000001";

pub(crate) fn build() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

pub(crate) fn template(catalog: &Value) -> Result<&Value, String> {
    catalog["contractTemplates"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|template| template["guid"] == TEMPLATE_GUID)
        .ok_or_else(|| "the template".to_owned())
}

#[test]
fn a_contract_template_guid_resolves_to_an_entry_and_a_leftover_is_left_out() -> TestResult {
    let catalog = build()?;
    let templates = catalog["contractTemplates"].as_array().ok_or("templates")?;
    assert_eq!(
        templates.len(),
        2,
        "the template outside the folder is kept, the development leftover is left out"
    );
    for contract in catalog["contracts"].as_array().ok_or("contracts")? {
        assert_eq!(contract["templateGuid"], TEMPLATE_GUID);
    }
    let entry = template(&catalog)?;
    assert_eq!(entry["className"], "Hunt");
    assert_eq!(
        entry["recordPath"],
        "libs/foundry/records/contracts/contracttemplates/hunt"
    );
    Ok(())
}

#[test]
fn display_info_class_settings_and_deadline_are_exported_as_stored() -> TestResult {
    let catalog = build()?;
    let entry = template(&catalog)?;
    assert_eq!(entry["title"]["key"], "@place_name");
    assert_eq!(entry["title"]["en"], "Hijacked Ship");
    assert_eq!(
        entry["missionTypeGuid"],
        "50000000-0000-4000-8000-000000000001"
    );
    assert_eq!(
        (
            &entry["illegal"],
            &entry["showLifeTimeInMobiGlas"],
            &entry["preShowObjectives"],
            &entry["notForRelease"],
            &entry["workInProgress"],
        ),
        (
            &json!(true),
            &json!(false),
            &json!(true),
            &json!(false),
            &Value::Null
        )
    );
    assert_eq!(entry["classType"], "ContractClass_Contract");
    assert_eq!(entry["settings"]["canBeShared"], true);
    assert_eq!(entry["settings"]["hasCompleteButton"], false);
    assert_eq!(entry["autoFinish"]["failIfSentToPrison"], true);
    assert_eq!(entry["autoFinish"]["completionTime"], 300);
    assert_eq!(entry["autoFinish"]["autoEnd"], true);
    assert_eq!(entry["autoFinish"]["resultAfterTimerEnd"], "failed");
    assert_eq!(entry["autoFinish"]["remainingTimeToShowTimer"], 15);
    Ok(())
}

#[test]
fn properties_list_the_variables_with_the_kind_of_their_value() -> TestResult {
    let catalog = build()?;
    let properties: Vec<(&str, &str, &str)> = template(&catalog)?["contractProperties"]
        .as_array()
        .ok_or("properties")?
        .iter()
        .map(|property| {
            (
                property["variable"].as_str().unwrap_or_default(),
                property["token"].as_str().unwrap_or_default(),
                property["valueType"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        properties,
        [
            ("Wave_BP", "", "ShipSpawnDescriptions"),
            ("Place_BP", "Place", "Location"),
            ("Home_BP", "", "Location"),
            ("Flag_BP", "", "Boolean"),
            ("Cargo_BP", "Cargo", "HaulingOrders"),
        ]
    );
    Ok(())
}

#[test]
fn ship_spawns_and_location_queries_link_to_the_shared_kinds_at_the_template_level() -> TestResult {
    let catalog = build()?;
    let entry = template(&catalog)?;
    let variables = |field: &str| -> Vec<(String, String)> {
        entry[field]
            .as_array()
            .into_iter()
            .flatten()
            .map(|link| {
                (
                    link["variable"].as_str().unwrap_or_default().to_owned(),
                    link["level"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect()
    };
    assert_eq!(
        variables("shipSpawns"),
        [("Wave_BP".to_owned(), "template".to_owned())]
    );
    assert_eq!(
        variables("locationQueries"),
        [
            ("Home_BP".to_owned(), "template".to_owned()),
            ("Place_BP".to_owned(), "template".to_owned())
        ]
    );
    let pool = &entry["shipSpawns"][0]["poolGuid"];
    assert!(
        catalog["contractShipSpawns"]
            .as_array()
            .ok_or("pools")?
            .iter()
            .any(|spawn| &spawn["guid"] == pool),
        "the link resolves"
    );
    Ok(())
}

#[test]
fn hauling_orders_name_the_property_and_the_objective_source() -> TestResult {
    let catalog = build()?;
    let orders = template(&catalog)?["haulingOrders"]
        .as_array()
        .ok_or("orders")?;
    let sources: Vec<(&str, Option<&str>)> = orders
        .iter()
        .map(|order| {
            (
                order["source"].as_str().unwrap_or_default(),
                order["variableName"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        sources,
        [("template", Some("Cargo_BP")), ("templateObjective", None)]
    );
    let item = &orders[1]["requirements"][0]["options"][0][0];
    assert_eq!(item["maxScu"], 8);
    Ok(())
}

#[test]
fn objectives_follow_the_phases_depth_first_and_skip_a_token_without_an_id() -> TestResult {
    let catalog = build()?;
    let objectives = template(&catalog)?["objectives"]
        .as_array()
        .ok_or("objectives")?;
    let ids: Vec<(&str, Option<&str>, &str)> = objectives
        .iter()
        .map(|objective| {
            (
                objective["id"].as_str().unwrap_or_default(),
                objective["parentId"].as_str(),
                objective["debugName"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        ids,
        [
            ("70000000-0000-4000-8000-000000000001", None, "Deliver"),
            (
                "70000000-0000-4000-8000-000000000002",
                Some("70000000-0000-4000-8000-000000000001"),
                ""
            ),
            ("70000000-0000-4000-8000-000000000003", None, "Go"),
        ],
        "the token without an id is left out"
    );
    assert_eq!(objectives[0]["startsActive"], true);
    assert_eq!(objectives[1]["startsActive"], false);
    assert_eq!(objectives[1]["phaseTag"], "Kill");
    assert_eq!(objectives[0]["phaseTag"], Value::Null);
    assert_eq!(objectives[1]["variableName"], "PhaseB");
    assert_eq!(objectives[1]["weight"], 0.5);
    assert_eq!(objectives[0]["variableName"], Value::Null);
    Ok(())
}
