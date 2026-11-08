// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The crafting stations in the Basetool catalog, through the public API.

use crate::catalog_crafting_support::commodity;
use crate::crafters_fixture::{
    BENCH, BROKEN, CATEGORY_A, CATEGORY_B, FABRICATOR, HOST_SHIP, MODIFIED, MOUNTED, ODD,
    SELF_SHIP, write_archive,
};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = p4kx_export::CatalogPublicationMode::DiagnosticPartial;
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn class_names<'a>(catalog: &'a Value, kind: &str) -> Vec<&'a str> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["className"].as_str())
        .collect()
}

#[test]
fn every_entity_with_a_crafter_component_is_a_crafter_whatever_its_kind() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog, "crafters"),
        [
            "bench",
            "fabricator",
            "crafting_item",
            "crafting_commodity",
            "acme_mounted",
            "acme_self",
            "modified",
            "odd",
            "broken"
        ],
        "ordered by GUID; the entity without the component and the template are absent"
    );
    assert!(
        class_names(&catalog, "items").contains(&"crafting_item"),
        "a crafting item stays an item"
    );
    assert!(
        commodity::class_names(&catalog)?.contains(&"crafting_commodity"),
        "a crafting commodity stays a commodity"
    );
    let kind = built.summary().kind(Kind::Crafters);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((9, 1, true)),
        "the template is the one skipped; the plain template holds no component"
    );
    Ok(())
}

#[test]
fn a_crafter_carries_the_limits_of_its_component_as_stored() -> TestResult {
    let (_, catalog) = build(None)?;
    let entry = catalog["crafters"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == guid(BENCH).as_str())
        .ok_or("the crafter")?;
    assert_eq!(
        *entry,
        json!({
            "guid": guid(BENCH),
            "className": "bench",
            "recordPath": "libs/foundry/records/entities/basebuilding/interactables/bench",
            "type": null,
            "subType": null,
            "size": null,
            "maxJobsInHistory": 10,
            "baseCraftingTime": 0.0,
            "queues": [
                {
                    "name": "Fabricator",
                    "maxJobsInProgress": 2,
                    "maxJobsWaiting": 4,
                    "blueprintCategoryAvailability": no_rule(),
                    "processTypeAvailability": [],
                },
                {
                    "name": "Workbench",
                    "maxJobsInProgress": 4,
                    "maxJobsWaiting": 3,
                    "blueprintCategoryAvailability": no_rule(),
                    "processTypeAvailability": [],
                },
            ],
            "modifiers": [],
            "issues": [],
            "hostGuids": [],
        })
    );
    Ok(())
}

fn crafter(catalog: &Value, value: u128) -> Result<&Value, Box<dyn Error>> {
    catalog["crafters"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == guid(value).as_str())
        .ok_or_else(|| "the crafter".into())
}

#[test]
fn a_crafter_without_queues_has_an_empty_list() -> TestResult {
    let (_, catalog) = build(None)?;
    let entry = catalog["crafters"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == guid(FABRICATOR).as_str())
        .ok_or("the crafter")?;
    assert_eq!(entry["queues"], json!([]));
    assert_eq!(entry["maxJobsInHistory"], 0);
    Ok(())
}

#[test]
fn a_filter_keeps_the_crafters_whose_path_it_matches() -> TestResult {
    let filter = Filter::new("*/basebuilding/*");
    let (built, catalog) = build(Some(&filter))?;
    assert_eq!(
        class_names(&catalog, "crafters"),
        ["bench", "fabricator", "modified", "odd", "broken"],
        "the item and the commodity are outside the filter"
    );
    assert_eq!(
        catalog["completeness"]["crafters"],
        json!({"status": "partial", "filter": "*/basebuilding/*"})
    );
    assert!(
        built
            .summary()
            .kind(Kind::Crafters)
            .is_some_and(|kind| !kind.complete),
        "the summary agrees"
    );
    Ok(())
}

#[test]
fn a_ship_hosts_the_crafter_its_default_loadout_mounts_through_a_nested_port() -> TestResult {
    let (_, catalog) = build(None)?;
    let mounted = crafter(&catalog, MOUNTED)?;
    assert_eq!(mounted["hostGuids"], json!([guid(HOST_SHIP)]));
    assert_eq!(mounted["queues"][0]["name"], "Mounted queue");
    assert!(
        class_names(&catalog, "items").contains(&"acme_mounted"),
        "the mounted crafter is an item as well"
    );
    Ok(())
}

#[test]
fn a_ship_that_is_a_crafter_stays_a_ship_and_is_matched_by_guid_not_hosted() -> TestResult {
    let (_, catalog) = build(None)?;
    let own = crafter(&catalog, SELF_SHIP)?;
    assert_eq!(own["hostGuids"], json!([]), "a ship does not host itself");
    assert_eq!(own["maxJobsInHistory"], 8);
    let ships = class_names(&catalog, "ships");
    assert!(ships.contains(&"acme_self"), "{ships:?}");
    assert!(ships.contains(&"acme_empty"), "{ships:?}");
    Ok(())
}

fn no_rule() -> Value {
    json!({"kind": "none", "structName": null, "blueprintCategoryGuids": []})
}

#[test]
fn a_time_modifier_with_a_category_condition_and_the_queue_rules_are_exported_as_stored()
-> TestResult {
    let (_, catalog) = build(None)?;
    let entry = crafter(&catalog, MODIFIED)?;
    assert_eq!(
        (&entry["type"], &entry["subType"], &entry["size"]),
        (&json!("Crafter"), &json!("Fabricator"), &json!(2)),
        "the attach definition a port is matched against"
    );
    assert_eq!(entry["baseCraftingTime"], 2.5);
    assert_eq!(
        entry["modifiers"],
        json!([
            {
                "type": "time",
                "structName": "CraftingModifierType_Time",
                "timeMultiplier": 0.75,
                "condition": {
                    "kind": "blueprintCategory",
                    "structName": "CraftingModifierCondition_BlueprintCategory",
                    "blueprintCategoryGuids": [guid(CATEGORY_B)],
                },
            },
            {
                "type": "time",
                "structName": "CraftingModifierType_Time",
                "timeMultiplier": 0.5,
                "condition": no_rule(),
            },
        ])
    );
    assert_eq!(
        entry["queues"][0]["blueprintCategoryAvailability"],
        json!({
            "kind": "whitelist",
            "structName": "BlueprintCategoryAvailability_Whitelist",
            "blueprintCategoryGuids": [guid(CATEGORY_A), guid(CATEGORY_B)],
        })
    );
    assert_eq!(
        entry["queues"][0]["processTypeAvailability"],
        json!([true, false, true])
    );
    assert_eq!(entry["issues"], json!([]));
    let categories = class_names(&catalog, "blueprintCategories");
    assert_eq!(
        categories,
        ["category_a", "category_b"],
        "the GUIDs resolve to entries of blueprintCategories"
    );
    Ok(())
}

#[test]
fn an_unknown_type_condition_and_availability_are_kept_by_name_without_values() -> TestResult {
    let (_, catalog) = build(None)?;
    let entry = crafter(&catalog, ODD)?;
    assert_eq!(
        entry["modifiers"],
        json!([
            {
                "type": "unknown",
                "structName": "CraftingModifierType_Yield",
                "timeMultiplier": null,
                "condition": {
                    "kind": "blueprintCategory",
                    "structName": "CraftingModifierCondition_BlueprintCategory",
                    "blueprintCategoryGuids": [guid(CATEGORY_A)],
                },
            },
            {
                "type": "time",
                "structName": "CraftingModifierType_Time",
                "timeMultiplier": 0.9,
                "condition": {
                    "kind": "unknown",
                    "structName": "CraftingModifierCondition_Daytime",
                    "blueprintCategoryGuids": [],
                },
            },
        ])
    );
    assert_eq!(
        entry["queues"][0]["blueprintCategoryAvailability"],
        json!({
            "kind": "unknown",
            "structName": "BlueprintCategoryAvailability_Blacklist",
            "blueprintCategoryGuids": [],
        })
    );
    assert_eq!(
        entry["issues"],
        json!([
            "unknown_category_availability",
            "unknown_modifier_condition",
            "unknown_modifier_type"
        ])
    );
    Ok(())
}

#[test]
fn a_modifier_without_a_type_or_a_number_is_kept_and_reported() -> TestResult {
    let (_, catalog) = build(None)?;
    let entry = crafter(&catalog, BROKEN)?;
    assert_eq!(
        entry["modifiers"],
        json!([
            {
                "type": "none",
                "structName": null,
                "timeMultiplier": null,
                "condition": no_rule(),
            },
            {
                "type": "time",
                "structName": "CraftingModifierType_Time",
                "timeMultiplier": null,
                "condition": no_rule(),
            },
        ])
    );
    assert_eq!(
        entry["issues"],
        json!([
            "missing_modifier_type",
            "unreadable_category",
            "unreadable_time_multiplier"
        ])
    );
    Ok(())
}

#[test]
fn a_crafter_without_modifiers_has_none_and_no_issue() -> TestResult {
    let (_, catalog) = build(None)?;
    let entry = crafter(&catalog, FABRICATOR)?;
    assert_eq!(entry["modifiers"], json!([]));
    assert_eq!(entry["issues"], json!([]));
    Ok(())
}
