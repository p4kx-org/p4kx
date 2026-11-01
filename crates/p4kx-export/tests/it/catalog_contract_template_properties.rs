// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The properties of the objectives of the contract templates, a nil tag, the records the catalog
//! does not follow to their end and the stored names it does not know.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CraftingWarning, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, template};
use crate::spawns_fixture;

type TestResult = Result<(), Box<dyn Error>>;

const OBJECTIVE_A: &str = "70000000-0000-4000-8000-000000000001";

fn field<'c>(entry: &'c Value, pointer: &str) -> &'c Value {
    entry.pointer(pointer).unwrap_or(&Value::Null)
}

fn properties(catalog: &Value, objective: usize) -> Result<&Vec<Value>, String> {
    template(catalog)?
        .pointer(&format!("/objectives/{objective}/objectiveProperties"))
        .and_then(Value::as_array)
        .ok_or_else(|| format!("properties of objective {objective}"))
}

fn kind_guids<'c>(catalog: &'c Value, kind: &str) -> Vec<&'c str> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["guid"].as_str())
        .collect()
}

#[test]
fn a_referenced_property_names_the_property_of_the_template_it_points_to() -> TestResult {
    let catalog = build()?;
    let referenced = &properties(&catalog, 0)?[0];
    assert_eq!(referenced["kind"], "referenced");
    assert_eq!(referenced["variable"], "Cargo_BP");
    assert_eq!(referenced["token"], "Cargo");
    assert_eq!(referenced["contractProperty"], "Place_BP");
    assert_eq!(referenced["value"], Value::Null);
    assert_eq!(referenced["linkedOutput"], Value::Null);
    Ok(())
}

#[test]
fn output_and_embedded_properties_state_their_values_by_type() -> TestResult {
    let catalog = build()?;
    let properties = properties(&catalog, 0)?;
    assert_eq!(properties.len(), 11);
    assert_eq!(properties[1]["kind"], "output");
    assert_eq!(properties[1]["variable"], "Out_BP");
    assert_eq!(properties[1]["value"], json!({"type": "object"}));
    assert_eq!(
        properties[2]["value"],
        json!({"type": "boolean", "value": true})
    );
    assert_eq!(
        properties[3]["value"],
        json!({"type": "stringHash", "options": [{
            "displayName": {"key": "@place_name", "en": "Hijacked Ship", "de": null},
            "value": "calm",
            "weighting": 3,
            "dependentProperties": ["Home_BP"]
        }]})
    );
    assert_eq!(
        properties[4]["value"],
        json!({"type": "integer", "options": [{
            "displayName": {"key": null, "en": null, "de": null},
            "value": 5.0,
            "variation": 1.0,
            "weighting": 2,
            "dependentProperties": []
        }]})
    );
    assert_eq!(
        properties[5]["value"],
        json!({"type": "tags", "tags": ["Kill"], "negativeTags": []})
    );
    Ok(())
}

#[test]
fn npc_entity_and_organization_values_link_or_state_their_fields() -> TestResult {
    let catalog = build()?;
    let properties = properties(&catalog, 0)?;
    assert_eq!(field(&properties[6], "/value/type"), "npcSpawnDescriptions");
    let npc = field(&properties[6], "/value/poolGuid")
        .as_str()
        .ok_or("npc pool guid")?;
    assert!(kind_guids(&catalog, "contractNpcSpawns").contains(&npc));
    assert_eq!(
        field(&properties[9], "/value/type"),
        "entitySpawnDescriptions"
    );
    let entity = field(&properties[9], "/value/poolGuid")
        .as_str()
        .ok_or("entity pool guid")?;
    assert!(kind_guids(&catalog, "contractEntitySpawns").contains(&entity));
    assert_eq!(
        properties[10]["value"],
        json!({
            "type": "organization",
            "conditions": [{
                "condition": "specificOrganizations",
                "organizationGuids": ["80000000-0000-4000-8000-000000000001"]
            }],
            "forcedOrganizationGuid": "80000000-0000-4000-8000-000000000001"
        })
    );
    Ok(())
}

#[test]
fn a_ship_spawn_or_location_value_links_to_the_shared_kinds() -> TestResult {
    let catalog = build()?;
    let properties = properties(&catalog, 0)?;
    let pool = field(&properties[7], "/value/poolGuid")
        .as_str()
        .ok_or("pool guid")?;
    assert!(kind_guids(&catalog, "contractShipSpawns").contains(&pool));
    assert_eq!(
        field(&properties[7], "/value/type"),
        "shipSpawnDescriptions"
    );
    let query = field(&properties[8], "/value/queryGuid")
        .as_str()
        .ok_or("query guid")?;
    assert!(kind_guids(&catalog, "contractLocationQueries").contains(&query));
    assert_eq!(field(&properties[8], "/value/type"), "location");
    Ok(())
}

#[test]
fn an_input_property_names_the_output_and_the_objective_it_takes() -> TestResult {
    let catalog = build()?;
    let input = &properties(&catalog, 1)?[0];
    assert_eq!(input["kind"], "input");
    assert_eq!(input["variable"], "In_BP");
    assert_eq!(input["value"], json!({"type": "deliveryOrder"}));
    assert_eq!(
        input["linkedOutput"],
        json!({"objectiveId": OBJECTIVE_A, "variable": "Out_BP"})
    );
    Ok(())
}

#[test]
fn a_nil_tag_is_no_tag_path_in_a_list_or_in_a_single_tag_field() -> TestResult {
    let catalog = build()?;
    let templates = catalog["missionLocationTemplates"]
        .as_array()
        .ok_or("templates")?;
    assert_eq!(
        templates[0]["tags"],
        json!(["Missions.Hijacked"]),
        "the nil tag in the list is left out"
    );
    let phase = field(template(&catalog)?, "/objectives/2/phaseTag");
    assert_eq!(phase, &Value::Null, "the nil tag of a single tag field");
    Ok(())
}

#[test]
fn records_that_nest_phases_or_conditions_too_deep_fail_instead_of_being_cut() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    let built = build_catalog(&archive, &options)?;
    let mut failed: Vec<(&str, &str)> = built
        .summary()
        .failures()
        .iter()
        .map(|failure| (failure.record_path.as_str(), failure.reason.as_str()))
        .collect();
    failed.sort_unstable();
    assert_eq!(failed.len(), 7, "{failed:?}");
    assert!(failed[0].0.ends_with("deep_flow"), "{failed:?}");
    assert!(failed[0].1.contains("conditions nest deeper than 16"));
    assert!(failed[1].0.ends_with("deep_phases"), "{failed:?}");
    assert!(failed[1].1.contains("phases nest deeper than 32"));
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        catalog["contractTemplates"].as_array().map(Vec::len),
        Some(2),
        "no failed template is listed"
    );
    Ok(())
}

#[test]
fn a_struct_the_catalog_does_not_know_fails_the_record_instead_of_hiding_behind_other() -> TestResult
{
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    let built = build_catalog(&archive, &options)?;
    let mut failed: Vec<(&str, &str)> = built
        .summary()
        .failures()
        .iter()
        .map(|failure| (failure.record_path.as_str(), failure.reason.as_str()))
        .collect();
    failed.sort_unstable();
    assert!(failed[2].0.ends_with("unknown_condition"), "{failed:?}");
    assert!(
        failed[2].1.contains(
            "organization condition holds the unknown struct DataSetMatchCondition_Mystery"
        ),
        "{failed:?}"
    );
    assert!(failed[3].0.ends_with("unknown_flow"), "{failed:?}");
    assert!(
        failed[3]
            .1
            .contains("holds the unknown struct MissionFlowCondition_Unknown"),
        "{failed:?}"
    );
    assert!(failed[4].0.ends_with("unknown_modifier"), "{failed:?}");
    assert!(
        failed[4]
            .1
            .contains("modifiers holds the unknown struct MissionModifier"),
        "{failed:?}"
    );
    assert!(failed[5].0.ends_with("unknown_value"), "{failed:?}");
    assert!(
        failed[5]
            .1
            .contains("property value holds the unknown struct MissionPropertyValue_Mystery"),
        "{failed:?}"
    );
    assert!(failed[6].0.ends_with("missionbroker/bad"), "{failed:?}");
    assert!(
        failed[6]
            .1
            .contains("holds the unknown struct SReputationMissionRequirementsUnknown"),
        "{failed:?}"
    );
    Ok(())
}

#[test]
fn stored_names_the_catalog_does_not_know_are_counted_in_a_warning() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    let built = build_catalog(&archive, &options)?;
    let warning = built
        .summary()
        .warnings()
        .iter()
        .find(|warning| warning.kind() == "unknown_template_values")
        .ok_or("the warning")?;
    assert_eq!(warning.count(), Some(5), "{:?}", warning.details());
    assert_eq!(
        warning.details(),
        [
            "comparison Sideways (1)",
            "end comms reason Vanish (1)",
            "flow state Sleeping (1)",
            "month Smarch (1)",
            "reaction Gloating (1)"
        ]
    );
    assert!(matches!(
        warning,
        CraftingWarning::UnknownTemplateValues { .. }
    ));
    Ok(())
}
