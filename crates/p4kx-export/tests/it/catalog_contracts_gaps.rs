// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The places, the giver faction and the blueprint rewards of a contract in the Basetool catalog.

use crate::catalog_contracts::{A, B, C, E, SUB_ONE, TestResult, build, entry};

use serde_json::{Value, json};

const D: &str = "20000000-0000-4000-8000-000000000005";
const G: &str = "20000000-0000-4000-8000-000000000008";
const H: &str = "20000000-0000-4000-8000-000000000009";
const I: &str = "20000000-0000-4000-8000-00000000000a";
const SUB_TWO: &str = "30000000-0000-4000-8000-000000000002";
const POOL_CONTRACT: &str = "e4000000-0000-4000-8000-000000000001";
const POOL_MODIFIER: &str = "e4000000-0000-4000-8000-000000000004";
const STRAY: &str = "60000000-0000-4000-8000-000000000005";
const POOL_SUB: &str = "e4000000-0000-4000-8000-000000000003";
const LOC_A: &str = "a0000000-0000-4000-8000-000000000001";
const LOC_B: &str = "a0000000-0000-4000-8000-000000000002";
const ACME: &str = "60000000-0000-4000-8000-000000000001";
const MISSING_OBJECT: &str = "c2000000-0000-4000-8000-000000000001";

fn names(places: &Value) -> Vec<(&str, &str)> {
    places
        .as_array()
        .into_iter()
        .flatten()
        .map(|place| {
            (
                place["objectType"].as_str().unwrap_or("none"),
                place
                    .get("place")
                    .and_then(|name| name.get("en"))
                    .and_then(Value::as_str)
                    .unwrap_or("none"),
            )
        })
        .collect()
}

#[test]
fn a_contract_links_its_localities_and_a_sub_contract_replaces_them() -> TestResult {
    let (_, catalog) = build(None)?;
    let contract = &entry(&catalog, A, None)?["offeredAt"];
    assert_eq!(contract["localityGuids"], json!([LOC_A]));
    assert_eq!(contract["locations"], json!([]));
    let sub = &entry(&catalog, A, Some(SUB_ONE))?["offeredAt"];
    assert_eq!(sub["localityGuids"], json!([]), "its own place replaces");
    assert_eq!(names(&sub["locations"]), [("Planet", "Hurston")]);
    Ok(())
}

#[test]
fn a_locality_and_its_pickup_places_retain_every_resolved_object() -> TestResult {
    let (_, catalog) = build(None)?;
    let contract = entry(&catalog, B, None)?;
    assert_eq!(contract["pickupPlaces"].as_array().map(Vec::len), Some(7));
    assert_eq!(contract["offeredAt"]["localityGuids"], json!([LOC_B]));
    let localities = catalog["missionLocalities"]
        .as_array()
        .ok_or("localities")?;
    let all = localities
        .iter()
        .find(|locality| locality["guid"] == LOC_B)
        .ok_or("locality")?;
    assert_eq!(all["starmapObjectGuids"].as_array().map(Vec::len), Some(7));
    Ok(())
}

#[test]
fn locations_are_listed_as_stated_with_stars_and_unknown_objects() -> TestResult {
    let (_, catalog) = build(None)?;
    let offered = &entry(&catalog, D, None)?["offeredAt"];
    assert_eq!(
        offered["locations"],
        json!([
            {
                "starmapObjectGuid": "c0000000-0000-4000-8000-000000000003",
                "objectType": "Star",
                "place": {"key": "@place_sol", "en": "Sol", "de": null},
            },
            {
                "starmapObjectGuid": MISSING_OBJECT,
                "objectType": null,
                "place": {"key": null, "en": null, "de": null},
            },
        ]),
        "a star stays, an unknown object keeps its GUID, none twice"
    );
    assert_eq!(
        entry(&catalog, D, None)?["pickupPlaces"],
        json!([]),
        "the pickup places stay as they are"
    );
    Ok(())
}

#[test]
fn a_contract_without_a_place_takes_the_location_of_its_broker_entry() -> TestResult {
    let (_, catalog) = build(None)?;
    let broker = &entry(&catalog, E, None)?["offeredAt"];
    assert_eq!(names(&broker["locations"]), [("Planet", "Hurston")]);
    assert_eq!(broker["localityGuids"], json!([]));
    assert_eq!(
        broker["brokerFallback"], true,
        "told apart from a stated one"
    );
    assert_eq!(
        entry(&catalog, C, None)?["offeredAt"],
        json!({"localityGuids": [], "locations": [], "brokerFallback": false})
    );
    assert_eq!(
        entry(&catalog, D, None)?["offeredAt"]["brokerFallback"],
        false
    );
    Ok(())
}

#[test]
fn the_giver_faction_is_the_handlers_and_names_its_source() -> TestResult {
    let (_, catalog) = build(None)?;
    let contract = entry(&catalog, A, None)?;
    assert_eq!(
        contract["giverFaction"],
        json!({"source": "handler", "faction": contract["faction"]})
    );
    Ok(())
}

#[test]
fn without_a_handler_faction_a_positive_amount_names_the_giver_and_a_rival_does_not() -> TestResult
{
    let (_, catalog) = build(None)?;
    let contract = entry(&catalog, I, None)?;
    assert_eq!(contract["faction"], Value::Null);
    assert_eq!(contract["giverFaction"]["source"], "reputationResult");
    assert_eq!(contract["giverFaction"]["faction"]["guid"], ACME);
    assert_eq!(contract["giverFaction"]["faction"]["name"], "acme");
    assert_eq!(
        contract["giverFaction"]["faction"]["displayName"]["en"],
        "Acme Emporium"
    );
    Ok(())
}

#[test]
fn a_rival_with_a_negative_amount_falls_through_to_the_prerequisite() -> TestResult {
    let (_, catalog) = build(None)?;
    let contract = entry(&catalog, H, None)?;
    assert_eq!(contract["giverFaction"]["source"], "reputationPrerequisite");
    let faction = &contract["giverFaction"]["faction"];
    assert_eq!(
        faction["guid"], STRAY,
        "a faction outside the faction folder stays"
    );
    assert_eq!(
        faction["name"], "odd",
        "the name comes from the reference path"
    );
    assert_eq!(faction["displayName"]["en"], Value::Null);
    Ok(())
}

#[test]
fn a_sub_contract_shares_the_contracts_blueprint_rewards_unless_it_states_its_own() -> TestResult {
    let (_, catalog) = build(None)?;
    let contract = json!([{"poolGuid": POOL_CONTRACT, "source": "result", "chance": 0.25}]);
    assert_eq!(
        entry(&catalog, A, None)?["blueprintRewards"],
        contract,
        "only the reward that fires on success"
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_ONE))?["blueprintRewards"],
        contract
    );
    assert_eq!(
        entry(&catalog, A, Some(SUB_TWO))?["blueprintRewards"],
        json!([{"poolGuid": POOL_SUB, "source": "result", "chance": 1.0}]),
        "a sub contract with its own results does not take the contract's"
    );
    assert_eq!(entry(&catalog, B, None)?["blueprintRewards"], json!([]));
    assert_eq!(
        entry(&catalog, C, None)?["blueprintRewards"],
        json!([{"poolGuid": POOL_MODIFIER, "source": "temporaryModifier", "chance": null}]),
        "an enabled temporary blueprint modifier, not the disabled one"
    );
    Ok(())
}

#[test]
fn an_exclusion_and_a_contract_without_a_faction_have_no_giver() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(entry(&catalog, G, None)?["giverFaction"], Value::Null);
    assert_eq!(entry(&catalog, C, None)?["giverFaction"], Value::Null);
    Ok(())
}
