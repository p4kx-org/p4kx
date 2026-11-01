// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! NPC and entity spawn descriptions in the Basetool catalog.

use std::error::Error;

use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, template};

type TestResult = Result<(), Box<dyn Error>>;

const BROKER: &str = "b0000000-0000-4000-8000-000000000001";
const OWNER: &str = "80000000-0000-4000-8000-000000000001";

fn contract(catalog: &Value, sub: bool) -> Result<&Value, String> {
    catalog["contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["subContractGuid"].is_null() != sub)
        .ok_or_else(|| "the contract entry".to_owned())
}

fn broker(catalog: &Value) -> Result<&Value, String> {
    catalog["missionBrokerEntries"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == BROKER)
        .ok_or_else(|| "the broker entry".to_owned())
}

fn pool<'c>(catalog: &'c Value, kind: &str, link: &Value) -> Result<&'c Value, String> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .find(|pool| pool["guid"] == link["poolGuid"])
        .ok_or_else(|| format!("{kind} {link}"))
}

/// The variable, the level and the name of the first group of every link in `field`.
fn links(catalog: &Value, entry: &Value, field: &str) -> Result<Vec<String>, String> {
    let kind = if field == "npcSpawns" {
        "contractNpcSpawns"
    } else {
        "contractEntitySpawns"
    };
    entry[field]
        .as_array()
        .into_iter()
        .flatten()
        .map(|link| {
            Ok(format!(
                "{}@{} {}",
                link["variable"].as_str().unwrap_or_default(),
                link["level"].as_str().unwrap_or_default(),
                pool(catalog, kind, link)?
                    .pointer("/groups/0/name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            ))
        })
        .collect()
}

#[test]
fn the_nearest_level_that_states_a_variable_wins_and_an_empty_description_is_left_out() -> TestResult
{
    let catalog = build()?;
    let expected = ["Crew_BP@contract Squad", "Guard_BP@template Guards"];
    assert_eq!(
        links(&catalog, contract(&catalog, false)?, "npcSpawns")?,
        expected
    );
    assert_eq!(
        links(&catalog, contract(&catalog, true)?, "npcSpawns")?,
        expected
    );
    assert_eq!(
        links(&catalog, contract(&catalog, false)?, "entitySpawns")?,
        ["Loot_BP@handler Crate"],
        "the handler's entity description hides the template's"
    );
    Ok(())
}

#[test]
fn a_template_and_a_broker_entry_link_their_own_descriptions() -> TestResult {
    let catalog = build()?;
    let template = template(&catalog)?;
    assert_eq!(
        links(&catalog, template, "npcSpawns")?,
        ["Guard_BP@template Guards", "Spare_BP@template Spares"]
    );
    assert_eq!(
        links(&catalog, template, "entitySpawns")?,
        ["Loot_BP@template Template crate"]
    );
    let broker = broker(&catalog)?;
    assert_eq!(
        links(&catalog, broker, "npcSpawns")?,
        ["Crew_BP@broker Squad"]
    );
    assert_eq!(
        links(&catalog, broker, "entitySpawns")?,
        ["Loot_BP@broker Crate"]
    );
    Ok(())
}

#[test]
fn equal_descriptions_share_one_entry_whoever_states_them() -> TestResult {
    let catalog = build()?;
    let names = |kind: &str| -> Vec<&str> {
        catalog[kind]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|pool| pool["groups"][0]["name"].as_str())
            .collect()
    };
    let mut npcs = names("contractNpcSpawns");
    npcs.sort_unstable();
    assert_eq!(
        npcs,
        ["Guards", "Reserve", "Spares", "Squad"],
        "Squad is stated by a contract, a broker entry and an objective, and listed once"
    );
    let mut entities = names("contractEntitySpawns");
    entities.sort_unstable();
    assert_eq!(entities, ["Crate", "Template crate"]);
    let guids: Vec<&str> = catalog["contractNpcSpawns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|pool| pool["guid"].as_str())
        .collect();
    let mut ordered = guids.clone();
    ordered.sort_unstable();
    assert_eq!(guids, ordered, "ordered by identifier");
    Ok(())
}

#[test]
fn an_npc_description_states_every_field_of_the_records() -> TestResult {
    let catalog = build()?;
    let squad = pool(
        &catalog,
        "contractNpcSpawns",
        &contract(&catalog, false)?["npcSpawns"][0],
    )?;
    assert_eq!(
        squad["groups"][0]["options"][0],
        json!({
            "priority": 2,
            "includeLocationAISpawnTags": true,
            "weight": 0.5,
            "autoSpawnSettings": {
                "name": "Squad (Grenadier x1)",
                "initialActivity": "Sentry",
                "excludeShipCrew": true,
                "excludeSpawnGender": "Female",
                "minGroupSize": -1,
                "maxGroupSize": 4,
                "maxConcurrentSpawns": 4,
                "maxSpawns": 6,
                "minSpawnDelay": 0,
                "maxSpawnDelay": 10,
                "missionAlliedMarker": false,
                "isCritical": true,
                "characterTags": {"positiveTags": ["ShipModel"], "negativeTags": ["Kill"]},
                "closetTags": {"positiveTags": ["Missions.Hijacked"], "negativeTags": []},
                "roomTags": {"positiveTags": [], "negativeTags": []},
                "defendAreaTags": {"positiveTags": ["Kill"], "negativeTags": []},
                "scheduleAreaTags": {"positiveTags": [], "negativeTags": ["ShipModel"]},
                "entityTags": ["Missions.Hijacked"],
                "inventoryItems": [{
                    "entityClassGuid": OWNER,
                    "quantity": 3,
                    "amountInventoriesToFill": 1,
                    "markAsMissionItem": true
                }],
                "factionOverrideGuid": OWNER
            },
            "identifierTags": ["Kill"],
            "namePropertyVariable": null,
            "embeddedName": {
                "randomName": true,
                "randomLastName": false,
                "randomNickName": true,
                "givenName": {"key": "@place_name", "en": "Hijacked Ship", "de": null},
                "givenLastName": {"key": null, "en": null, "de": null},
                "givenNickName": {"key": null, "en": null, "de": null},
                "chanceOfNickName": 0.25,
                "nameDataGuid": OWNER
            }
        })
    );
    Ok(())
}

#[test]
fn a_name_property_pointer_is_the_variable_of_the_property_it_points_to() -> TestResult {
    let catalog = build()?;
    let guards = pool(
        &catalog,
        "contractNpcSpawns",
        &contract(&catalog, false)?["npcSpawns"][1],
    )?;
    assert_eq!(
        guards["groups"][0]["options"][0]["namePropertyVariable"],
        "Place_BP"
    );
    assert_eq!(
        guards["groups"][0]["options"][0]["embeddedName"],
        Value::Null
    );
    Ok(())
}

#[test]
fn an_entity_description_states_groups_slots_and_options() -> TestResult {
    let catalog = build()?;
    let crate_pool = pool(
        &catalog,
        "contractEntitySpawns",
        &contract(&catalog, false)?["entitySpawns"][0],
    )?;
    assert_eq!(
        crate_pool["groups"],
        json!([{
            "name": "Crate",
            "slots": [{"options": [{
                "amount": 10,
                "weight": 0.15,
                "tags": ["ShipModel"],
                "negativeTags": [],
                "markupTags": ["Kill"],
                "entityTags": []
            }]}]
        }])
    );
    Ok(())
}
