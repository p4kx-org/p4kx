// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Ship spawns, location queries and location templates in the P4KX catalog.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

use crate::spawns_fixture;

#[path = "catalog_mission_location_sdk.rs"]
mod sdk;

type TestResult = Result<(), Box<dyn Error>>;

fn build() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
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
        weights(
            &catalog,
            crate::catalog_contracts::entry_by_level(&catalog, false)?
        )?,
        [("Wave_BP@contract".to_owned(), 2.0)],
        "the empty Empty_BP of the contract hides the handler's and is itself left out"
    );
    assert_eq!(
        weights(
            &catalog,
            crate::catalog_contracts::entry_by_level(&catalog, true)?
        )?,
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
    assert_eq!(
        option["crewInheritsLocationFaction"], false,
        "the authored false crew-faction flag survives"
    );
    assert_eq!(option["tags"], serde_json::json!(["ShipModel"]));
    assert_eq!(pools[0]["groups"][0]["name"], "Wave");
    Ok(())
}

#[test]
fn ship_options_keep_the_crew_faction_flag_for_each_authored_alternative() -> TestResult {
    let catalog = build()?;
    let pools = catalog["contractShipSpawns"].as_array().ok_or("pools")?;
    for pool in pools {
        let option = pool
            .pointer("/groups/0/slots/0/options/0")
            .ok_or("option")?;
        assert_eq!(
            option["crewInheritsLocationFaction"],
            option["weight"] == 3.0,
            "the flag follows its source option independently of shared pool links"
        );
    }
    Ok(())
}

#[test]
fn location_queries_are_exported_as_stored_with_tag_paths() -> TestResult {
    let catalog = build()?;
    let contract = crate::catalog_contracts::entry_by_level(&catalog, false)?;
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
    let levels: Vec<(&str, &str)> =
        crate::catalog_contracts::entry_by_level(&catalog, true)?["locationPrerequisites"]
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

fn check_location_template_names(catalog: &Value, template: &Value) -> TestResult {
    use crate::health_origin_assertions as proof;
    use crate::text_forms_assertions::direct;

    let [first, second, placeholder] = proof::rows(template, "/names")? else {
        return Err("expected two named variants and one placeholder".into());
    };
    assert_eq!(
        proof::at(first, "/tag/path/value")?.as_str(),
        Some("Missions.Hijacked"),
        "the first variant keeps its original tag",
    );
    assert_eq!(
        proof::at(second, "/tag/path/value")?.as_str(),
        Some("ShipModel"),
        "the repeated key keeps its own tag",
    );
    let first = proof::at(first, "/displayName")?;
    let second = proof::at(second, "/displayName")?;
    let placeholder = proof::at(placeholder, "/displayName")?;
    for reference in [first, second, placeholder] {
        direct(reference)?;
        assert_eq!(
            proof::at(reference, "/containsEncryptedSource")?.as_bool(),
            Some(true),
            "each reference comes from the encrypted template",
        );
    }
    assert_eq!(
        proof::at(first, "/state")?.as_str(),
        Some("present"),
        "resolved name"
    );
    let identifier = proof::at(first, "/value")?;
    assert_eq!(
        identifier,
        proof::at(second, "/value")?,
        "repeated key shares a text row"
    );
    assert_ne!(
        proof::at(first, "/sourceUsesRefs")?,
        proof::at(second, "/sourceUsesRefs")?,
        "the repeated key cites its distinct authored occurrence",
    );
    assert_eq!(
        proof::at(placeholder, "/state")?.as_str(),
        Some("absent"),
        "placeholder state"
    );
    assert!(
        proof::at(placeholder, "/value")?.is_null(),
        "placeholder has no reference"
    );
    check_location_name_forms(catalog, identifier)?;
    proof::closure(catalog)
}

fn check_location_name_forms(catalog: &Value, identifier: &Value) -> TestResult {
    use crate::health_origin_assertions as proof;
    use crate::text_forms_assertions::{forms, present};

    let row = proof::rows(catalog, "/texts/gameTexts")?
        .iter()
        .find(|row| row.pointer("/id/value") == Some(identifier))
        .ok_or("protected location name")?;
    assert_eq!(
        proof::at(row, "/key/value")?.as_str(),
        Some("@place_name"),
        "original key"
    );
    present(forms(row, "en")?, "displayText", "Hijacked Ship")?;
    present(forms(row, "fr")?, "displayText", "Site de mission")?;
    assert!(
        row.pointer("/translations/value/de").is_none(),
        "German has no source entry"
    );
    Ok(())
}

#[test]
fn location_templates_carry_their_tags_and_leave_out_leftovers() -> TestResult {
    let catalog = build()?;
    let templates = catalog["missionLocationTemplates"]
        .as_array()
        .ok_or("templates")?;
    assert_eq!(templates.len(), 2, "the dev leftover is left out");
    let first = templates.first().ok_or("first location template")?;
    sdk::verify_wire_tags(&catalog, first)?;
    check_location_template_names(&catalog, first)?;
    assert!(
        templates
            .iter()
            .any(|template| template["disabled"] == true),
        "the disabled flag is kept as stored"
    );
    Ok(())
}
