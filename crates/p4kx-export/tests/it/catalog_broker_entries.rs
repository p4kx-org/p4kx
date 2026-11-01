// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mission broker entries in the Basetool catalog.

use std::error::Error;

use serde_json::{Value, json};

use crate::catalog_contract_templates::build;

type TestResult = Result<(), Box<dyn Error>>;

const BROKER: &str = "b0000000-0000-4000-8000-000000000001";
const ELSEWHERE: &str = "b0000000-0000-4000-8000-000000000002";
const MISSION_TYPE: &str = "50000000-0000-4000-8000-000000000001";
const OWNER: &str = "80000000-0000-4000-8000-000000000001";
const SCOPE: &str = "80000000-0000-4000-8000-000000000002";

fn entry(catalog: &Value) -> Result<&Value, String> {
    catalog["missionBrokerEntries"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == BROKER)
        .ok_or_else(|| "the broker entry".to_owned())
}

#[test]
fn an_entry_outside_the_folder_is_kept_a_leftover_is_left_out_and_a_contract_links_to_its_entry()
-> TestResult {
    let catalog = build()?;
    let entries = catalog["missionBrokerEntries"]
        .as_array()
        .ok_or("entries")?;
    let guids: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry["guid"].as_str())
        .collect();
    assert_eq!(guids, [BROKER, ELSEWHERE], "ordered by GUID");
    for contract in catalog["contracts"].as_array().ok_or("contracts")? {
        assert_eq!(contract["brokerEntryGuid"], BROKER);
    }
    let broker = entry(&catalog)?;
    assert_eq!(broker["className"], "Haul_A");
    assert_eq!(
        broker["recordPath"],
        "libs/foundry/records/missionbroker/covalex/haul_a"
    );
    Ok(())
}

#[test]
fn texts_are_game_texts_and_a_placeholder_has_no_key() -> TestResult {
    let catalog = build()?;
    let broker = entry(&catalog)?;
    assert_eq!(broker["title"]["key"], "@place_name");
    assert_eq!(broker["title"]["en"], "Hijacked Ship");
    assert_eq!(broker["hudTitle"]["key"], Value::Null);
    assert_eq!(broker["missionGiver"]["en"], "Hijacked Ship");
    assert_eq!(broker["finish"]["endReason"]["en"], "Hijacked Ship");
    assert_eq!(broker["debugName"], "Haul A");
    Ok(())
}

#[test]
fn links_and_the_identity_of_the_giver_are_guids() -> TestResult {
    let catalog = build()?;
    let broker = entry(&catalog)?;
    assert_eq!(broker["missionTypeGuid"], MISSION_TYPE);
    assert_eq!(broker["locationGuid"], OWNER);
    assert_eq!(broker["localityGuid"], Value::Null);
    assert_eq!(broker["invitationMissionGuid"], ELSEWHERE);
    assert_eq!(broker["linkedMissionGuid"], Value::Null);
    assert_eq!(broker["associatedMissionGuids"], json!([ELSEWHERE]));
    assert_eq!(broker["requiredMissionGuids"], json!([ELSEWHERE]));
    assert_eq!(broker["owner"], json!({"guid": OWNER, "className": "MDT"}));
    assert_eq!(broker["giverFragmentTags"], "Covalex");
    assert_eq!(broker["module"], "Libs/Subsumption/Missions/Haul.xml");
    Ok(())
}

#[test]
fn terms_are_exported_as_stored() -> TestResult {
    let catalog = build()?;
    let broker = entry(&catalog)?;
    assert_eq!(broker["difficulty"], 3);
    assert_eq!(broker["buyInAmount"], 500);
    assert_eq!(broker["refundBuyInOnWithdraw"], true);
    assert_eq!(broker["settings"]["canBeShared"], true);
    assert_eq!(broker["settings"]["onlyOwnerCanComplete"], true);
    assert_eq!(broker["availability"]["initiallyActive"], true);
    assert_eq!(broker["availability"]["wantedLevelMax"], 2.0);
    assert_eq!(broker["availability"]["requestOnly"], false);
    assert_eq!(broker["cooldowns"]["personalCooldownTime"], 60.0);
    assert_eq!(broker["cooldowns"]["respawnTime"], 10.0);
    assert_eq!(broker["instances"]["maxInstances"], 4);
    assert_eq!(broker["instances"]["lifeTime"], 120.0);
    assert_eq!(
        broker["reward"],
        json!({
            "currency": "UEC",
            "reward": 12000,
            "max": 15000,
            "plusBonuses": true,
            "reputationBonusGuid": SCOPE
        })
    );
    let finish = &broker["finish"];
    assert_eq!(finish["autoEnd"], true);
    assert_eq!(finish["completionTime"], 45.5);
    assert_eq!(finish["resultAfterTimerEnd"], "failed");
    assert_eq!(finish["failIfBecameCriminal"], true);
    Ok(())
}

#[test]
fn the_schedule_names_its_dates_and_nulls_a_month_it_does_not_know() -> TestResult {
    let catalog = build()?;
    assert_eq!(
        entry(&catalog)?["schedule"],
        json!([{
            "start": {"year": 2950, "month": "december", "day": 1, "hour": 8, "minute": 30, "second": 0},
            "end": {"year": 2951, "month": "january", "day": 8, "hour": 17, "minute": 30, "second": 0},
            "repeatEnd": {"year": 2999, "month": null, "day": 1, "hour": null, "minute": null, "second": null},
            "repeatType": "repeatYearly"
        }])
    );
    Ok(())
}

#[test]
fn tags_are_paths_and_required_missions_are_terms() -> TestResult {
    let catalog = build()?;
    let broker = entry(&catalog)?;
    assert_eq!(broker["completionTags"], json!(["Kill"]));
    assert_eq!(broker["missionTags"], json!(["Missions.Hijacked"]));
    assert_eq!(broker["requiredAreaTags"], json!([]));
    assert_eq!(
        broker["requiredCompletedTags"],
        json!([{"positiveTags": ["Kill"], "negativeTags": ["Missions.Hijacked"]}])
    );
    assert_eq!(
        broker["journalEntriesOnComplete"].as_array().map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn the_reputation_requirements_are_an_expression_and_the_results_name_their_amounts() -> TestResult
{
    let catalog = build()?;
    let broker = entry(&catalog)?;
    let requirement = |comparison: Value| {
        json!({
            "type": "requirement",
            "comparison": comparison,
            "factionReputationGuid": OWNER,
            "scopeGuid": SCOPE,
            "standingGuid": MISSION_TYPE
        })
    };
    assert_eq!(
        broker["standingRequirements"],
        json!([
            requirement(json!("greaterThanOrEqualTo")),
            {"type": "and"},
            {"type": "leftParenthesis"},
            requirement(Value::Null),
            {"type": "rightParenthesis"}
        ])
    );
    assert_eq!(
        broker["reputationResults"],
        json!([
            {"index": 0, "amounts": [{
                "factionReputationGuid": OWNER,
                "scopeGuid": SCOPE,
                "rewardGuid": "b1000000-0000-4000-8000-000000000001",
                "amount": 2
            }]},
            {"index": 1, "amounts": []}
        ])
    );
    Ok(())
}

#[test]
fn properties_spawns_and_orders_are_read_like_those_of_a_template() -> TestResult {
    let catalog = build()?;
    let broker = entry(&catalog)?;
    assert_eq!(
        broker["missionProperties"],
        json!([
            {"variable": "Wave_BP", "token": null, "valueType": "ShipSpawnDescriptions"},
            {"variable": "Place_BP", "token": "Place", "valueType": "Location"}
        ])
    );
    let level = |field: &str| broker[field][0]["level"].clone();
    assert_eq!(level("shipSpawns"), "broker");
    assert_eq!(level("locationQueries"), "broker");
    assert!(
        catalog["contractShipSpawns"]
            .as_array()
            .ok_or("pools")?
            .iter()
            .any(|pool| pool["guid"] == broker["shipSpawns"][0]["poolGuid"]),
        "the link resolves"
    );
    let orders = broker["haulingOrders"].as_array().ok_or("orders")?;
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0]["source"], "brokerObjective");
    assert_eq!(orders[0]["requirements"][0]["options"][0][0]["maxScu"], 8);
    Ok(())
}

#[test]
fn objectives_payout_flow_and_modifiers_use_the_shapes_of_the_template() -> TestResult {
    let catalog = build()?;
    let broker = entry(&catalog)?;
    let objectives = broker["objectives"].as_array().ok_or("objectives")?;
    assert_eq!(objectives.len(), 1);
    assert_eq!(objectives[0]["debugName"], "Haul");
    assert_eq!(
        objectives[0]["handler"]["structName"],
        "ObjectiveHandler_Hauling"
    );
    assert_eq!(
        broker["partialPayout"]["thresholds"][0]["minPercentage"],
        76
    );
    assert_eq!(
        broker["missionFlow"][0]["condition"],
        json!({
            "type": "tokenState",
            "objectiveId": objectives[0]["id"],
            "tokenState": "completed"
        })
    );
    assert_eq!(
        broker["modifiers"],
        json!([
            {
                "modifier": "factionHostility",
                "modifierName": "Rivals",
                "enabled": true,
                "locationProperty": null,
                "factionGuid": OWNER,
                "myReaction": "hostile",
                "theirReaction": null,
                "ignoreCriminalHostility": true
            },
            {
                "modifier": "hostileMission",
                "modifierName": null,
                "enabled": true,
                "locationProperty": null,
                "brokerEntryGuid": ELSEWHERE,
                "legalToAttack": true
            },
            {
                "modifier": "ignoreMissionPlayerCriminality",
                "modifierName": null,
                "enabled": true,
                "locationProperty": null,
                "brokerEntryGuid": ELSEWHERE
            },
            {
                "modifier": "lawLicense",
                "modifierName": null,
                "enabled": true,
                "locationProperty": null,
                "licenseType": "illegalGoodsLicenseClassB"
            }
        ])
    );
    Ok(())
}
