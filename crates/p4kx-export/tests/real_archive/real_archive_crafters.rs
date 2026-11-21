// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::catalog_v3_consumer;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const FABRICATOR_PATH: &str = "libs/foundry/records/entities/basebuilding/interactables/\
                               basebuilding_interactables_itemfabricator_standard";
const STATIONS: &str = "/definitions/crafters/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_crafting_stations_of_a_real_archive_carry_their_limits() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let crafters = catalog
        .pointer(STATIONS)
        .and_then(Value::as_array)
        .ok_or("the crafters")?;
    let kind = built
        .summary()
        .kind(Kind::Crafters)
        .ok_or("the crafters summary")?;
    assert_eq!(
        (kind.count, kind.skipped),
        (2, 1),
        "LIVE 4.10 source baseline"
    );
    let fabricator = crafters
        .iter()
        .find(|crafter| crafter["recordPath"]["value"] == FABRICATOR_PATH)
        .ok_or("the standard item fabricator")?;
    let component = &fabricator["component"]["value"];
    assert_eq!(
        component["maxJobsInHistory"]["value"]["value"], 10,
        "the original history limit"
    );
    let limits: Vec<Value> = component["queues"]["value"]
        .as_array()
        .ok_or("the queues")?
        .iter()
        .map(|queue| {
            json!([
                queue.pointer("/value/maxJobsInProgress/value/value"),
                queue.pointer("/value/maxJobsWaiting/value/value"),
            ])
        })
        .collect();
    assert_eq!(limits, [json!([2, 4]), json!([4, 4])], "queue limits");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_crafters_of_a_real_archive_resolve_their_categories_and_report_nothing_unreadable()
-> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let categories = catalog_v3_consumer::rows(&catalog, Kind::BlueprintCategories)?
        .iter()
        .map(|category| catalog_v3_consumer::identity(category, Kind::BlueprintCategories))
        .collect::<Result<HashSet<_>, _>>()?;
    let guids_resolve = |rule: &Value| {
        !matches!(
            rule.pointer("/value/kind").and_then(Value::as_str),
            Some("blueprintCategory" | "whitelist")
        ) || rule
            .pointer("/value/categories/value")
            .and_then(Value::as_array)
            .is_some_and(|slots| {
                slots.iter().all(|slot| {
                    slot.pointer("/value/guid/value")
                        .and_then(Value::as_str)
                        .is_some_and(|guid| categories.contains(guid))
                })
            })
    };
    for crafter in catalog
        .pointer(STATIONS)
        .and_then(Value::as_array)
        .ok_or("the crafters")?
    {
        let name = &crafter["className"]["value"];
        let component = &crafter["component"]["value"];
        let time = &component["baseCraftingTime"];
        assert!(
            matches!(time["state"].as_str(), Some("present" | "absent" | "null")),
            "{name}: baseCraftingTime state"
        );
        for modifier in retained_slots(&component["modifiers"])? {
            assert!(
                guids_resolve(&modifier["value"]["condition"]),
                "{name}: condition"
            );
        }
        for queue in retained_slots(&component["queues"])? {
            let queue = &queue["value"];
            assert!(
                guids_resolve(&queue["blueprintCategoryAvailability"]),
                "{name}: availability"
            );
            let flags = &queue["processTypeAvailability"];
            assert!(
                matches!(flags["state"].as_str(), Some("absent" | "null"))
                    || flags["value"].is_array(),
                "{name}: authored process slots"
            );
        }
    }
    Ok(())
}

fn retained_slots(cell: &Value) -> TestResult<&[Value]> {
    match cell["state"].as_str() {
        Some("absent" | "null") => Ok(&[]),
        Some("present") => cell["value"]
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| "the authored slots".into()),
        _ => Err("unreadable authored array".into()),
    }
}
