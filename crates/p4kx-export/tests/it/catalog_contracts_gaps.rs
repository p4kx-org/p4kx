// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The places, the giver faction and the blueprint rewards of a contract in the P4KX catalog.

use crate::catalog_contracts::{A, B, C, E, SUB_ONE, TestResult, build, entry, places};

use p4kx_export::Filter;

use crate::commodity_v3_fixture::text_row;

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

const STAR: &str = "c0000000-0000-4000-8000-000000000003";
const PLANET: &str = "c0000000-0000-4000-8000-000000000002";

fn starmap_name<'a>(catalog: &'a Value, guid: &str) -> TestResult<&'a Value> {
    let objects = catalog
        .pointer("/definitions/starmapObjects/rows")
        .and_then(Value::as_array)
        .ok_or("canonical starmap rows")?;
    objects
        .iter()
        .find(|object| object.pointer("/guid/value").and_then(Value::as_str) == Some(guid))
        .and_then(|object| object.get("name"))
        .ok_or_else(|| "canonical starmap name".into())
}

#[test]
fn a_contract_links_its_localities_and_a_sub_contract_replaces_them() -> TestResult {
    let (_, catalog) = build(None)?;
    let contract = &entry(&catalog, A, None)?["offeredAt"];
    assert_eq!(contract["localityGuids"], json!([LOC_A]));
    assert_eq!(contract["locations"], json!([]));
    let sub = entry(&catalog, A, Some(SUB_ONE))?;
    let offered = &sub["offeredAt"];
    assert_eq!(
        offered["localityGuids"],
        json!([]),
        "its own place replaces"
    );
    assert_eq!(
        places(&catalog, &offered["locations"])?,
        [("Planet", "Hurston")]
    );
    let name = starmap_name(&catalog, PLANET)?;
    assert_eq!(&offered["locations"][0]["place"], name);
    assert_eq!(&sub["pickupPlaces"][0]["place"], name);
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
                "starmapObjectGuid": STAR,
                "objectType": "Star",
                "place": starmap_name(&catalog, STAR)?,
            },
            {
                "starmapObjectGuid": MISSING_OBJECT,
                "objectType": null,
                "place": null,
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
fn a_filtered_starmap_object_retains_its_offered_guid_without_a_name() -> TestResult {
    let filter = Filter::new("contractgenerator");
    let (_, catalog) = build(Some(&filter))?;
    let sub = entry(&catalog, A, Some(SUB_ONE))?;
    assert_eq!(sub["pickupPlaces"], json!([]));
    assert_eq!(
        sub["offeredAt"]["locations"],
        json!([{"starmapObjectGuid": PLANET, "objectType": null, "place": null}])
    );
    Ok(())
}

#[test]
fn a_contract_without_a_place_takes_the_location_of_its_broker_entry() -> TestResult {
    let (_, catalog) = build(None)?;
    let broker = &entry(&catalog, E, None)?["offeredAt"];
    assert_eq!(
        places(&catalog, &broker["locations"])?,
        [("Planet", "Hurston")]
    );
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
    let display = &contract["giverFaction"]["faction"]["displayName"];
    let original = &entry(&catalog, A, None)?["faction"]["displayName"];
    assert_eq!(
        display, original,
        "both uses retain the same original name cell"
    );
    let factions = catalog
        .pointer("/definitions/factionReputations/rows")
        .and_then(Value::as_array)
        .ok_or("canonical faction reputation rows")?;
    let definition = factions
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(ACME))
        .ok_or("original faction definition")?;
    let canonical = &definition["displayName"];
    assert_eq!(
        display.get("value"),
        canonical.get("value"),
        "same protected text"
    );
    assert_eq!(
        display.get("containsEncryptedSource"),
        canonical.get("containsEncryptedSource"),
        "same original encryption state"
    );
    check_faction_name_source(&catalog, display, canonical)?;
    let text = text_row(&catalog, "gameTexts", &display["value"])?;
    assert_eq!(
        text.pointer("/translations/value/en/displayText/value")
            .and_then(Value::as_str),
        Some("Acme Emporium")
    );
    Ok(())
}

fn check_faction_name_source(catalog: &Value, display: &Value, canonical: &Value) -> TestResult {
    use crate::health_origin_assertions as origin;

    let name = origin::uses(catalog, display)?
        .into_iter()
        .find(|row| row.get("role").and_then(Value::as_str) == Some("payload"))
        .ok_or("original name payload")?;
    assert!(
        origin::uses(catalog, canonical)?.iter().any(|use_row| {
            use_row.get("source") == name.get("source")
                && use_row.get("location") == name.get("location")
                && use_row.get("scope").and_then(Value::as_str) == Some("payload")
        }),
        "the dependency and canonical text use the same original name field"
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
    assert_eq!(faction["displayName"], Value::Null);
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
