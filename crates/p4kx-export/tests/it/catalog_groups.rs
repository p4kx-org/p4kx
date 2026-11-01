// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resource groups and blueprint categories of the Basetool catalog, through the public API.

use crate::crafting_fixture;

use std::collections::HashSet;
use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        crafting_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions {
        filter,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn class_names<'a>(catalog: &'a Value, kind: &str) -> Vec<&'a str> {
    catalog[kind]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry["className"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn resource_groups_are_the_group_records_ordered_by_guid_not_the_resource_types() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog, "resourceGroups"),
        ["Metal", "Mineral", "Alloy"],
        "ResourceTypeGroup only, not the ResourceType Records or the database beside them"
    );
    let kind = built.summary().kind(Kind::ResourceGroups);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((3, 0, true)),
        "counts"
    );
    Ok(())
}

#[test]
fn a_resource_group_carries_its_name_and_the_group_that_lists_it() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        catalog["resourceGroups"][0],
        json!({
            "guid": "c1000000-0000-4000-8000-000000000001",
            "className": "Metal",
            "recordPath": "libs/foundry/records/resourcetypedatabase/resourcetypedatabase",
            "name": {"key": "@items_commodities_type_metal", "en": "Metal", "de": "Metall"},
            "parentGuid": null,
        }),
        "a top-level group"
    );
    assert_eq!(
        catalog["resourceGroups"][1]["name"],
        json!({"key": "@LOC_PLACEHOLDER", "en": null, "de": null}),
        "a placeholder name keeps the key and has no text"
    );
    assert_eq!(
        catalog["resourceGroups"][2]["parentGuid"], "c1000000-0000-4000-8000-000000000001",
        "Metal lists Alloy in its groups"
    );
    assert_eq!(
        catalog["resourceGroups"][2]["name"],
        json!({"key": "@items_commodities_type_alloy", "en": null, "de": null}),
        "a key the language files lack"
    );
    Ok(())
}

#[test]
fn blueprint_categories_are_the_category_records_without_leftovers() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog, "blueprintCategories"),
        ["Ammo", "FPSArmours"],
        "BlueprintCategoryRecord only, not the database, and not the leftover"
    );
    let kind = built.summary().kind(Kind::BlueprintCategories);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((2, 1, true)),
        "counts"
    );
    assert_eq!(
        catalog["blueprintCategories"][0],
        json!({
            "guid": "d1000000-0000-4000-8000-000000000001",
            "className": "Ammo",
            "recordPath": "libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase",
            "name": {"key": null, "en": null, "de": null},
        }),
        "the Record holds no name"
    );
    Ok(())
}

#[test]
fn every_type_guid_and_category_guid_resolves_to_an_entry() -> TestResult {
    let (_, catalog) = build(None)?;
    let guids = |kind: &str| -> HashSet<&str> {
        catalog[kind]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| entry["guid"].as_str())
            .collect()
    };
    let groups = guids("resourceGroups");
    let categories = guids("blueprintCategories");
    let types: Vec<&str> = catalog["commodities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["typeGuid"].as_str())
        .collect();
    assert!(!types.is_empty(), "the fixture has typed commodities");
    for guid in types {
        assert!(groups.contains(guid), "type {guid}");
    }
    for blueprint in catalog["blueprints"].as_array().into_iter().flatten() {
        let guid = blueprint["categoryGuid"].as_str().ok_or("a category")?;
        assert!(categories.contains(guid), "category {guid}");
    }
    Ok(())
}

#[test]
fn a_filter_marks_both_kinds_partial_and_selects_by_record_path() -> TestResult {
    let filter = Filter::new("*/blueprintcategories/*");
    let (built, catalog) = build(Some(&filter))?;
    assert_eq!(
        class_names(&catalog, "blueprintCategories"),
        ["Ammo", "FPSArmours"],
        "filtered by record path"
    );
    assert!(
        class_names(&catalog, "resourceGroups").is_empty(),
        "filtered"
    );
    for kind in ["resourceGroups", "blueprintCategories"] {
        assert_eq!(
            catalog["completeness"][kind],
            json!({"status": "partial", "filter": "*/blueprintcategories/*"}),
            "{kind} is partial"
        );
    }
    assert!(
        built.summary().kinds().iter().all(|kind| !kind.complete),
        "the summary agrees"
    );
    Ok(())
}

#[test]
#[expect(clippy::too_many_lines, reason = "the list of every kind in order")]
fn the_kinds_follow_the_existing_ones_in_the_catalog_order() {
    assert_eq!(
        Kind::ALL.map(Kind::name),
        [
            "manufacturers",
            "items",
            "ships",
            "commodities",
            "blueprints",
            "resourceGroups",
            "blueprintCategories",
            "gameplayProperties",
            "qualityDistributions",
            "qualityQuantizations",
            "blueprintPools",
            "dismantleProcesses",
            "paints",
            "loadouts",
            "starmapObjects",
            "contracts",
            "mineableElements",
            "refiningProcesses",
            "mineableCompositions",
            "miningGlobalParams",
            "harvestableProviderPresets",
            "subHarvestableMultiConfigs",
            "harvestableLocations",
            "shops",
            "factionReputations",
            "reputationScopes",
            "reputationStandings",
            "reputationContexts",
            "missionTypes",
            "missionLocalities",
            "contractScenarios",
            "lootTables",
            "lootArchetypes",
            "lootPoolFilters",
            "lootContainers",
            "mineableEntities",
            "legacyLootTables",
            "legacyLootArchetypes",
            "crafters",
            "cargoManifests",
            "missionLocationTemplates",
            "contractShipSpawns",
            "contractLocationQueries",
            "contractTemplates",
            "missionBrokerEntries",
            "contractNpcSpawns",
            "contractEntitySpawns",
        ],
        "order of the catalog"
    );
    assert_eq!(
        (
            Kind::ResourceGroups.rule().map(|rule| rule.type_name),
            Kind::BlueprintCategories.rule().map(|rule| rule.type_name)
        ),
        (Some("ResourceTypeGroup"), Some("BlueprintCategoryRecord")),
        "types"
    );
}
