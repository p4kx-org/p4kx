// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Ship spawns, location queries and location templates in the Basetool catalog.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::spawns_fixture;

type TestResult = Result<(), Box<dyn Error>>;

fn build() -> Result<Value, Box<dyn Error>> {
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

fn entry(catalog: &Value, sub: bool) -> Result<&Value, String> {
    catalog["contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["subContractGuid"].is_null() != sub)
        .ok_or_else(|| "the contract entry".to_owned())
}

fn pool<'c>(catalog: &'c Value, kind: &str, guid: &Value) -> Result<&'c Value, String> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .find(|pool| &pool["guid"] == guid)
        .ok_or_else(|| format!("{kind} {guid}"))
}

fn weights(catalog: &Value, entry: &Value) -> Result<Vec<(String, f64)>, String> {
    entry["shipSpawns"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|link| {
            let pool = pool(catalog, "contractShipSpawns", &link["poolGuid"])?;
            Ok((
                format!(
                    "{}@{}",
                    link["variable"].as_str().unwrap_or_default(),
                    link["level"].as_str().unwrap_or_default()
                ),
                pool.pointer("/groups/0/slots/0/options/0/weight")
                    .and_then(Value::as_f64)
                    .unwrap_or_default(),
            ))
        })
        .collect()
}

#[test]
fn the_nearest_level_that_states_a_variable_wins_and_an_empty_description_is_left_out() -> TestResult
{
    let catalog = build()?;
    assert_eq!(
        weights(&catalog, entry(&catalog, false)?)?,
        [("Wave_BP@contract".to_owned(), 2.0)],
        "the empty Empty_BP of the contract hides the handler's and is itself left out"
    );
    assert_eq!(
        weights(&catalog, entry(&catalog, true)?)?,
        [("Wave_BP@subContract".to_owned(), 3.0)]
    );
    Ok(())
}

#[test]
fn equal_ship_spawn_descriptions_share_one_entry() -> TestResult {
    let catalog = build()?;
    let pools = catalog["contractShipSpawns"].as_array().ok_or("pools")?;
    assert_eq!(
        pools.len(),
        5,
        "weights 1, 2, 3, the template's 9 and the 7 of an objective property"
    );
    let option = &pools[0]["groups"][0]["slots"][0]["options"][0];
    assert_eq!(option["concurrentAmount"], 2);
    assert_eq!(option["tags"], serde_json::json!(["ShipModel"]));
    assert_eq!(pools[0]["groups"][0]["name"], "Wave");
    Ok(())
}

#[test]
fn location_queries_are_exported_as_stored_with_tag_paths() -> TestResult {
    let catalog = build()?;
    let contract = entry(&catalog, false)?;
    let links = contract["locationQueries"].as_array().ok_or("links")?;
    let names: Vec<(&str, &str)> = links
        .iter()
        .map(|link| {
            (
                link["variable"].as_str().unwrap_or_default(),
                link["level"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(names, [("Home_BP", "template"), ("Place_BP", "contract")]);
    let many = pool(&catalog, "contractLocationQueries", &links[1]["queryGuid"])?;
    assert_eq!(many["count"], "many");
    assert_eq!(
        (&many["minLocations"], &many["maxLocations"]),
        (&1.into(), &3.into())
    );
    assert_eq!(many["resourceTags"], serde_json::json!(["Kill"]));
    let condition = &many["conditions"][0];
    assert_eq!(condition["condition"], "tagSearch");
    assert_eq!(
        condition["terms"][0]["positiveTags"],
        serde_json::json!(["Missions.Hijacked"])
    );
    assert_eq!(
        condition["terms"][0]["negativeTags"],
        serde_json::json!(["ShipModel"])
    );
    let one = pool(&catalog, "contractLocationQueries", &links[0]["queryGuid"])?;
    assert_eq!(one["count"], "one");
    assert_eq!(one["minLocations"], Value::Null);
    Ok(())
}

#[test]
fn location_prerequisites_are_listed_per_level_as_stored() -> TestResult {
    let catalog = build()?;
    let levels: Vec<(&str, &str)> = entry(&catalog, true)?["locationPrerequisites"]
        .as_array()
        .ok_or("prerequisites")?
        .iter()
        .map(|item| {
            (
                item["level"].as_str().unwrap_or_default(),
                item["locationLevelType"].as_str().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(levels, [("handler", "System"), ("subContract", "Planet")]);
    Ok(())
}

#[test]
fn location_templates_carry_their_tags_and_leave_out_leftovers() -> TestResult {
    let catalog = build()?;
    let templates = catalog["missionLocationTemplates"]
        .as_array()
        .ok_or("templates")?;
    assert_eq!(templates.len(), 2, "the dev leftover is left out");
    assert_eq!(
        templates[0]["tags"],
        serde_json::json!(["Missions.Hijacked"])
    );
    assert_eq!(
        templates[0]["producesTags"]["positiveTags"],
        serde_json::json!(["ShipModel"])
    );
    assert_eq!(
        templates[0]["consumesTags"]["negativeTags"],
        serde_json::json!(["Kill"])
    );
    assert_eq!(
        templates[0]["aiSpawnTags"],
        serde_json::json!(["ShipModel"])
    );
    assert_eq!(templates[0]["names"][0]["tag"], "Missions.Hijacked");
    assert!(
        templates
            .iter()
            .any(|template| template["disabled"] == true),
        "the disabled flag is kept as stored"
    );
    Ok(())
}
