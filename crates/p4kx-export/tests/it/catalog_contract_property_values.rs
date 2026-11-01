// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The values of the `contractProperties` of a contract template and of the `properties` of a
//! broker entry.

use std::error::Error;

use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, template};

type TestResult = Result<(), Box<dyn Error>>;

fn value<'c>(catalog: &'c Value, variable: &str) -> Result<&'c Value, String> {
    template(catalog)?["contractProperties"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|property| property["variable"] == variable)
        .map(|property| &property["value"])
        .ok_or_else(|| format!("property {variable}"))
}

fn guids<'c>(catalog: &'c Value, kind: &str) -> Vec<&'c str> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["guid"].as_str())
        .collect()
}

#[test]
fn spawns_and_locations_link_to_the_shared_kinds_like_those_of_an_objective() -> TestResult {
    let catalog = build()?;
    let links = [
        (
            "Wave_BP",
            "shipSpawnDescriptions",
            "poolGuid",
            "contractShipSpawns",
        ),
        (
            "Place_BP",
            "location",
            "queryGuid",
            "contractLocationQueries",
        ),
        (
            "Guard_BP",
            "npcSpawnDescriptions",
            "poolGuid",
            "contractNpcSpawns",
        ),
        (
            "Loot_BP",
            "entitySpawnDescriptions",
            "poolGuid",
            "contractEntitySpawns",
        ),
    ];
    for (variable, kind, field, target) in links {
        let state = value(&catalog, variable)?;
        assert_eq!(state["type"], kind, "{variable}");
        let guid = state[field].as_str().ok_or(variable)?;
        assert!(guids(&catalog, target).contains(&guid), "{variable}");
    }
    Ok(())
}

#[test]
fn a_boolean_and_hauling_orders_state_their_content() -> TestResult {
    let catalog = build()?;
    assert_eq!(
        value(&catalog, "Flag_BP")?,
        &json!({"type": "boolean", "value": true})
    );
    let orders = value(&catalog, "Cargo_BP")?;
    assert_eq!(orders["type"], "haulingOrders");
    assert_eq!(
        orders["requirements"][0]["options"][0][0]["kind"],
        "resource"
    );
    assert_eq!(orders["requirements"][0]["options"][0][0]["maxScu"], 4);
    Ok(())
}

#[test]
fn a_mission_item_states_its_conditions_and_limits() -> TestResult {
    let catalog = build()?;
    let owner = "80000000-0000-4000-8000-000000000001";
    let item = value(&catalog, "Item_BP")?;
    assert_eq!(item["type"], "missionItem");
    assert_eq!(item["forcedItemGuid"], owner);
    assert_eq!(item["minItemsToFind"], 1);
    assert_eq!(item["maxItemsToFind"], 3);
    assert_eq!(item["conditions"][0]["condition"], "tagSearch");
    assert_eq!(
        item["conditions"][1],
        json!({"condition": "specificItems", "itemGuids": [owner]})
    );
    Ok(())
}

#[test]
fn a_name_states_its_texts_and_its_name_data() -> TestResult {
    let catalog = build()?;
    assert_eq!(
        value(&catalog, "Name_BP")?,
        &json!({
            "type": "aiName",
            "randomName": true,
            "randomLastName": false,
            "randomNickName": true,
            "givenName": {"key": "@place_name", "en": "Hijacked Ship", "de": null},
            "givenLastName": {"key": null, "en": null, "de": null},
            "givenNickName": {"key": null, "en": null, "de": null},
            "chanceOfNickName": 0.25,
            "nameDataGuid": "80000000-0000-4000-8000-000000000001"
        })
    );
    Ok(())
}

#[test]
fn combined_entries_a_reward_and_a_race_state_their_fields() -> TestResult {
    let catalog = build()?;
    assert_eq!(
        value(&catalog, "Combined_BP")?,
        &json!({"type": "combinedDataSetEntries", "entryProperties": ["Place_BP", "Home_BP"]})
    );
    assert_eq!(
        value(&catalog, "Pay_BP")?,
        &json!({"type": "reward", "reward": {
            "currency": "UEC",
            "reward": 900,
            "max": 1200,
            "plusBonuses": true,
            "reputationBonusGuid": "80000000-0000-4000-8000-000000000001"
        }})
    );
    assert_eq!(
        value(&catalog, "Race_BP")?,
        &json!({"type": "timeTrialRace"})
    );
    assert_eq!(
        value(&catalog, "Map_BP")?,
        &json!({"type": "starMapLocation"})
    );
    Ok(())
}
