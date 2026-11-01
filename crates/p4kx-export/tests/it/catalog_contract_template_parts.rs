// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The objective texts and handlers, the payout, modifiers, owner, mission flow and comms of the
//! contract templates in the Basetool catalog.

use std::error::Error;

use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, template};

type TestResult = Result<(), Box<dyn Error>>;

const OWNER: &str = "80000000-0000-4000-8000-000000000001";
const SCOPE: &str = "80000000-0000-4000-8000-000000000002";
const OBJECTIVE_A: &str = "70000000-0000-4000-8000-000000000001";
const OBJECTIVE_B: &str = "70000000-0000-4000-8000-000000000002";
const OBJECTIVE_C: &str = "70000000-0000-4000-8000-000000000003";

fn field<'c>(entry: &'c Value, pointer: &str) -> &'c Value {
    entry.pointer(pointer).unwrap_or(&Value::Null)
}

fn objective(catalog: &Value, index: usize) -> Result<&Value, String> {
    template(catalog)?["objectives"]
        .get(index)
        .ok_or_else(|| format!("objective {index}"))
}

#[test]
fn objective_texts_are_game_texts_with_the_german_text_missing_in_the_fixture() -> TestResult {
    let catalog = build()?;
    let first = objective(&catalog, 0)?;
    assert_eq!(field(first, "/display/category"), "primary");
    assert_eq!(field(first, "/display/hideOnHud"), false);
    assert_eq!(
        field(first, "/display/shortDescription"),
        &json!({"key": "@place_name", "en": "Hijacked Ship", "de": null})
    );
    assert_eq!(
        field(first, "/display/markerLabel"),
        &json!({"key": null, "en": null, "de": null}),
        "a placeholder key has no text"
    );
    assert_eq!(
        field(first, "/overrideDisplay/titleOverride/en"),
        "Hijacked Ship"
    );
    assert_eq!(
        field(first, "/overrideDisplay/descriptionOverride/key"),
        &Value::Null
    );
    Ok(())
}

#[test]
fn a_hauling_handler_states_its_markers_tokens_and_phrasings() -> TestResult {
    let catalog = build()?;
    let handler = field(objective(&catalog, 0)?, "/handler");
    assert_eq!(handler["structName"], "ObjectiveHandler_Hauling");
    assert_eq!(handler["hideMarkers"], true);
    assert_eq!(field(handler, "/hauling/itemToken"), "item");
    assert_eq!(
        field(handler, "/hauling/dropOffLocationToken"),
        "destination"
    );
    assert_eq!(
        field(handler, "/hauling/andObjective/shortDescription/en"),
        "Hijacked Ship"
    );
    assert_eq!(
        field(handler, "/hauling/collectObjective/shortDescription/key"),
        &Value::Null
    );
    assert_eq!(field(handler, "/hauling/orObjective"), &Value::Null);
    assert_eq!(handler["locationProperty"], Value::Null);
    assert_eq!(handler["instance"], Value::Null);
    Ok(())
}

#[test]
fn a_local_handler_states_its_radius_travel_text_and_marker_entity() -> TestResult {
    let catalog = build()?;
    let handler = field(objective(&catalog, 2)?, "/handler");
    assert_eq!(handler["structName"], "ObjectiveHandler_Local");
    assert_eq!(handler["module"], "Libs/Subsumption/Missions/Goto.xml");
    assert_eq!(handler["travelRadiusKm"], 2.5);
    assert_eq!(handler["disableTravelObjectives"], false);
    assert_eq!(
        field(handler, "/travelObjective/shortDescription/en"),
        "Hijacked Ship"
    );
    assert_eq!(field(handler, "/navPoint/entityClassGuid"), OWNER);
    assert_eq!(field(handler, "/navPoint/displayName/en"), "Hijacked Ship");
    assert_eq!(field(handler, "/navPoint/extendedTextTokens"), &json!([]));
    assert_eq!(handler["hauling"], Value::Null);
    Ok(())
}

#[test]
fn an_instance_handler_states_its_tags_and_text() -> TestResult {
    let catalog = build()?;
    let instance = field(objective(&catalog, 1)?, "/handler/instance");
    assert_eq!(instance["completePhaseOnPlayerEnter"], true);
    assert_eq!(instance["enterTravelRadiusKm"], 4.0);
    assert_eq!(field(instance, "/hudObjectiveText/en"), "Hijacked Ship");
    assert_eq!(instance["tagIdentifier"], "Kill");
    assert_eq!(instance["searchTags"], json!(["Kill"]));
    assert_eq!(instance["objectiveMarker"], "Marker");
    Ok(())
}

#[test]
fn reward_contributions_and_objective_comms_are_exported_as_stored() -> TestResult {
    let catalog = build()?;
    let first = objective(&catalog, 0)?;
    assert_eq!(
        first["rewardContribution"],
        json!({
            "structName": "ObjectiveRewardContribution_PercentOfMission",
            "percentContribution": 60,
            "doPartialRewardPayout": true
        })
    );
    assert_eq!(
        first["comms"],
        json!([{
            "trigger": "completed",
            "timing": "hudNotificationFirst",
            "delay": 2,
            "communicationTag": "Kill"
        }])
    );
    let second = objective(&catalog, 1)?;
    assert_eq!(second["rewardContribution"], Value::Null);
    assert_eq!(second["comms"], json!([]));
    Ok(())
}

#[test]
fn the_partial_payout_lists_its_ranges_with_the_scope_links() -> TestResult {
    let catalog = build()?;
    let range = field(template(&catalog)?, "/partialPayout/thresholds/0");
    assert_eq!(range["minPercentage"], 76);
    assert_eq!(range["maxPercentage"], 99);
    let currency = range["currencyRewardMultiplier"]
        .as_f64()
        .ok_or("currency")?;
    assert!((currency - 0.76).abs() < 1e-6, "{currency}");
    assert_eq!(
        range["reputationMultipliers"],
        json!([{"scopeGuid": SCOPE, "reputationRewardMultiplier": 2.0}]),
        "the GUID of a scope the Archive lacks is kept"
    );
    Ok(())
}

#[test]
fn modifiers_are_a_closed_list_tagged_by_their_struct() -> TestResult {
    let catalog = build()?;
    let modifiers = template(&catalog)?["modifiers"]
        .as_array()
        .ok_or("modifiers")?;
    assert_eq!(
        modifiers[0],
        json!({
            "modifier": "securityClearance",
            "modifierName": "GiveClearence",
            "enabled": true,
            "locationProperty": "Home_BP",
            "clearanceTokenGuid": OWNER
        })
    );
    assert_eq!(
        modifiers[1],
        json!({
            "modifier": "requestRentShip",
            "modifierName": "RentalMiner",
            "enabled": true,
            "locationProperty": null,
            "itemGuid": null,
            "durationSeconds": 3600,
            "clearRentalOnFail": true
        })
    );
    assert_eq!(
        modifiers[2],
        json!({
            "modifier": "shipKioskFilter",
            "modifierName": null,
            "enabled": false,
            "locationProperty": null,
            "terms": [{"positiveTags": ["Kill"], "negativeTags": []}]
        })
    );
    assert_eq!(
        modifiers[3],
        json!({
            "modifier": "lawLicense",
            "modifierName": null,
            "enabled": true,
            "locationProperty": null,
            "licenseType": "illegalGoodsLicenseClassA"
        })
    );
    Ok(())
}

#[test]
fn the_owner_names_its_record() -> TestResult {
    let catalog = build()?;
    assert_eq!(
        template(&catalog)?["owner"],
        json!({"guid": OWNER, "className": "MDT"})
    );
    Ok(())
}

#[test]
fn the_mission_flow_nests_its_conditions_and_nulls_unknown_states() -> TestResult {
    let catalog = build()?;
    let flow = template(&catalog)?["missionFlow"]
        .as_array()
        .ok_or("flow")?;
    assert_eq!(flow.len(), 8);
    assert_eq!(
        flow[6]["condition"],
        json!({"type": "anyTokensState", "tokenState": "failed"})
    );
    assert_eq!(flow[7]["condition"], json!({"type": "onMissionStart"}));
    assert_eq!(flow[0]["note"], "All done -> Completed");
    assert_eq!(
        flow[0]["condition"],
        json!({
            "type": "and",
            "conditions": [
                {"type": "tokenState", "objectiveId": OBJECTIVE_A, "tokenState": "completed"},
                {"type": "not", "condition":
                    {"type": "tokenState", "objectiveId": OBJECTIVE_B, "tokenState": "failed"}}
            ]
        })
    );
    assert_eq!(
        flow[0]["action"],
        json!({"type": "stateChange", "toState": "completed"})
    );
    assert_eq!(
        flow[1]["action"],
        json!({
            "type": "pickRandomMissionPhase",
            "phases": [{"objectiveId": OBJECTIVE_C, "probability": 0.25}],
            "toState": "active"
        })
    );
    assert_eq!(
        flow[1]["condition"],
        json!({"type": "tokenState", "objectiveId": null, "tokenState": "active"}),
        "an unset weak pointer is null"
    );
    assert_eq!(
        flow[2]["condition"],
        json!({"type": "allTokensState", "tokenState": "completed"})
    );
    assert_eq!(
        flow[2]["action"],
        json!({"type": "stateChange", "toState": null}),
        "a state the catalog does not know is null"
    );
    Ok(())
}

#[test]
fn a_weak_pointer_names_the_objective_or_the_property_or_is_null() -> TestResult {
    let catalog = build()?;
    let flow = template(&catalog)?["missionFlow"]
        .as_array()
        .ok_or("flow")?;
    assert_eq!(
        field(&flow[0], "/condition/conditions/0/objectiveId"),
        OBJECTIVE_A,
        "a top level token"
    );
    assert_eq!(
        field(&flow[0], "/condition/conditions/1/condition/objectiveId"),
        OBJECTIVE_B,
        "a phase below a token"
    );
    assert_eq!(
        field(&flow[3], "/condition/objectiveId"),
        &Value::Null,
        "a token with a nil id"
    );
    assert_eq!(
        flow[4]["condition"],
        json!({"type": "propertyIsSet", "propertyVariable": "Place_BP"}),
        "a property, by its variable name"
    );
    assert_eq!(
        field(&flow[5], "/condition/objectiveId"),
        &Value::Null,
        "an instance that is no part of the Record"
    );
    let modifier = field(template(&catalog)?, "/modifiers/0/locationProperty");
    assert_eq!(modifier, "Home_BP");
    let handler = field(objective(&catalog, 2)?, "/handler");
    assert_eq!(handler["locationProperty"], "Place_BP");
    Ok(())
}

#[test]
fn the_start_and_end_comms_name_their_tags_and_reasons() -> TestResult {
    let catalog = build()?;
    let entry = template(&catalog)?;
    assert_eq!(
        entry["startComms"],
        json!({"delay": 3, "communicationTag": "Kill"})
    );
    assert_eq!(
        entry["endComms"],
        json!([
            {"reason": "complete", "delay": 5, "communicationTag": "Kill"},
            {"reason": null, "delay": 0, "communicationTag": null}
        ])
    );
    Ok(())
}
