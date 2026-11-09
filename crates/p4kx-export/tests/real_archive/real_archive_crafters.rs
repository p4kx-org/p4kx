// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The crafting stations of the Basetool catalog of a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one.
//!
//! The anchors are the values of LIVE 4.10 and follow each patch; a failure after a patch means
//! the value moved, to be checked against the Archive and updated here and in
//! `docs/basetool-catalog.md`.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

const FABRICATOR_PATH: &str = "libs/foundry/records/entities/basebuilding/interactables/\
                               basebuilding_interactables_itemfabricator_standard";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_crafting_stations_of_a_real_archive_carry_their_limits() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let crafters = catalog["crafters"].as_array().ok_or("the crafters")?;
    let kind = built
        .summary()
        .kind(Kind::Crafters)
        .ok_or("the crafters summary")?;
    assert_eq!(
        (kind.count, kind.skipped),
        (2, 1),
        "three stations on LIVE 4.10, one of them a template"
    );
    let fabricator = crafters
        .iter()
        .find(|crafter| crafter["recordPath"] == FABRICATOR_PATH)
        .ok_or("the standard item fabricator")?;
    assert_eq!(fabricator["maxJobsInHistory"], 10);
    let limits: Vec<Value> = fabricator["queues"]
        .as_array()
        .ok_or("the queues")?
        .iter()
        .map(|queue| json!([queue["maxJobsInProgress"], queue["maxJobsWaiting"]]))
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
            rule["kind"].as_str(),
            Some("blueprintCategory" | "whitelist")
        ) || rule["blueprintCategoryGuids"]
            .as_array()
            .is_some_and(|guids| {
                guids
                    .iter()
                    .all(|guid| guid.as_str().is_some_and(|guid| categories.contains(guid)))
            })
    };
    for crafter in catalog["crafters"].as_array().ok_or("the crafters")? {
        let name = &crafter["className"];
        let issues = crafter["issues"].as_array().ok_or("the issues")?;
        for issue in issues {
            let code = issue.as_str().ok_or("an issue code")?;
            assert!(!code.starts_with("unreadable_"), "{name}: {code}");
        }
        assert!(
            crafter["baseCraftingTime"].is_null() || crafter["baseCraftingTime"].is_number(),
            "{name}: baseCraftingTime"
        );
        for modifier in crafter["modifiers"].as_array().ok_or("the modifiers")? {
            assert!(guids_resolve(&modifier["condition"]), "{name}: condition");
        }
        for queue in crafter["queues"].as_array().ok_or("the queues")? {
            assert!(
                guids_resolve(&queue["blueprintCategoryAvailability"]),
                "{name}: availability"
            );
            assert!(queue["processTypeAvailability"].is_array(), "{name}");
        }
    }
    Ok(())
}
