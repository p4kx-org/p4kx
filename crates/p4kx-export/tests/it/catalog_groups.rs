// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resource groups and blueprint categories of the Basetool catalog, through the public API.

use crate::catalog_crafting_support::commodity::{self, TextDomain};
use crate::catalog_crafting_support::{blueprint_categories, groups};
use crate::commodity_v3_fixture::at;
use crate::crafting_fixture;

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

fn class_names<'a>(catalog: &'a Value, kind: &str) -> Result<Vec<&'a str>, Box<dyn Error>> {
    let entries = match kind {
        "resourceGroups" => groups::definitions(catalog)?,
        "blueprintCategories" => blueprint_categories::rows(catalog)?,
        _ => return Err("unknown group/category fixture family".into()),
    };
    entries
        .iter()
        .map(|entry| {
            crate::commodity_v3_fixture::payload(entry, "className")?
                .as_str()
                .ok_or_else(|| "required canonical class name".into())
        })
        .collect()
}

fn assert_reference(reference: &Value, guid: &str, index: u32, kind: &str) -> TestResult {
    assert_eq!(
        reference
            .pointer("/state")
            .ok_or("required reference /state")?,
        &json!("present"),
        "the membership is present"
    );
    assert_eq!(
        reference
            .pointer("/value/guid/value")
            .ok_or("required reference /value/guid/value")?,
        &json!(guid),
        "the authored GUID stays intact"
    );
    assert_eq!(
        reference
            .pointer("/value/targetRecordIndex/value")
            .ok_or("required reference /value/targetRecordIndex/value")?,
        &json!(index),
        "the physical target stays intact"
    );
    assert_eq!(
        reference
            .pointer("/value/targetType/value")
            .ok_or("required reference /value/targetType/value")?,
        &json!(kind),
        "the target family stays intact"
    );
    assert_eq!(
        reference
            .pointer("/value/recordPath/value")
            .ok_or("required reference /value/recordPath/value")?,
        &json!("libs/foundry/records/resourcetypedatabase/resourcetypedatabase"),
        "the original canonical target path stays intact"
    );
    assert_eq!(
        reference
            .pointer("/value/status")
            .ok_or("required reference /value/status")?,
        &json!("present"),
        "source resolution stays explicit"
    );
    Ok(())
}

#[test]
fn resource_groups_are_the_group_records_ordered_by_guid_not_the_resource_types() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog, "resourceGroups")?,
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
    assert_metal_definition(&catalog)?;
    assert_metal_members(&catalog)?;
    assert_other_group_texts(&catalog)?;
    Ok(())
}

#[test]
fn blueprint_categories_are_the_category_records_without_leftovers() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        class_names(&catalog, "blueprintCategories")?,
        ["Ammo", "FPSArmours"],
        "BlueprintCategoryRecord only, not the database, and not the leftover"
    );
    let kind = built.summary().kind(Kind::BlueprintCategories);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped, kind.complete)),
        Some((2, 1, true)),
        "counts"
    );
    let row = blueprint_categories::rows(&catalog)?
        .first()
        .ok_or("the Ammo category")?;
    blueprint_categories::assert_header(
        row,
        &crafting_fixture::database(),
        "d1000000-0000-4000-8000-000000000001",
        "Ammo",
        "libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase",
    )?;
    Ok(())
}

#[test]
fn every_type_guid_and_category_guid_resolves_to_an_entry() -> TestResult {
    let (_, catalog) = build(None)?;
    let groups = groups::guids(&catalog)?;
    let categories = blueprint_categories::guids(&catalog)?;
    let types: Vec<&str> = catalog["projections"]["commodityPresentation"]["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["typeGuid"]["value"].as_str())
        .collect();
    assert!(!types.is_empty(), "the fixture has typed commodities");
    for guid in types {
        assert!(groups.contains(guid), "type {guid}");
    }
    for blueprint in catalog["projections"]["blueprintPresentation"]["rows"]
        .as_array()
        .ok_or("Blueprint presentations")?
    {
        let guid = blueprint["category"]["value"]
            .as_str()
            .ok_or("a category")?;
        assert!(categories.contains(guid), "category {guid}");
    }
    Ok(())
}

#[test]
fn a_filter_marks_both_kinds_partial_and_selects_by_record_path() -> TestResult {
    let filter = Filter::new("*/blueprintcategories/*");
    let (built, catalog) = build(Some(&filter))?;
    assert_eq!(
        class_names(&catalog, "blueprintCategories")?,
        ["Ammo", "FPSArmours"],
        "filtered by record path"
    );
    assert!(
        class_names(&catalog, "resourceGroups")?.is_empty(),
        "filtered"
    );
    for kind in ["resourceGroups", "blueprintCategories"] {
        assert_eq!(
            catalog["completeness"][kind],
            json!({"status": "partial", "filter": "*/blueprintcategories/*"}),
            "{kind} is partial"
        );
    }
    assert_eq!(
        catalog["completeness"]["tags"],
        json!({"status": "complete"}),
        "the global Tag taxonomy retains its full selection scope"
    );
    assert!(
        built
            .summary()
            .kinds()
            .iter()
            .all(|entry| entry.complete == matches!(entry.kind, Kind::Tags | Kind::ShopInventories)),
        "the Tag taxonomy and physical shop inventories are globally selected"
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
            "missionPropertySets",
            "resourceTypes",
            "tags",
            "blueprintCategoryLists",
            "shopInventories",
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

fn assert_metal_definition(catalog: &Value) -> TestResult {
    let metal = groups::definition(catalog, "c1000000-0000-4000-8000-000000000001")?;
    assert_eq!(
        at(metal, "/className/value")?,
        "Metal",
        "the original class name remains"
    );
    assert_eq!(
        at(metal, "/recordPath/value")?,
        "libs/foundry/records/resourcetypedatabase/resourcetypedatabase",
        "the original owner path remains"
    );
    assert!(
        at(metal, "/defaultThumbnailPath/value")?.is_null(),
        "the undeclared thumbnail remains without payload"
    );
    assert!(
        at(metal, "/defaultThumbnailPathSVG/value")?.is_null(),
        "the undeclared SVG remains without payload"
    );
    commodity::assert_text(
        catalog,
        at(metal, "/name")?,
        TextDomain::PublicName,
        Some("@items_commodities_type_metal"),
        Some("Metal"),
        Some("Metall"),
    )?;
    commodity::assert_text(
        catalog,
        at(metal, "/description")?,
        TextDomain::GameText,
        None,
        None,
        None,
    )?;
    Ok(())
}

fn assert_metal_members(catalog: &Value) -> TestResult {
    let members = groups::members(catalog, "c1000000-0000-4000-8000-000000000001")?;
    assert_reference(
        at(members, "/groups/value/0")?,
        "c3000000-0000-4000-8000-000000000003",
        6,
        "ResourceTypeGroup",
    )?;
    assert_reference(
        at(members, "/resources/value/0")?,
        "b1000000-0000-4000-8000-000000000001",
        0,
        "ResourceType",
    )?;
    assert_reference(
        at(members, "/resources/value/1")?,
        "b2000000-0000-4000-8000-000000000002",
        1,
        "ResourceType",
    )?;
    assert_eq!(
        at(members, "/groups/value")?.as_array().map(Vec::len),
        Some(1),
        "one original child occurrence"
    );
    assert_eq!(
        at(members, "/resources/value")?.as_array().map(Vec::len),
        Some(2),
        "two original resource occurrences"
    );
    assert!(
        groups::parent_guid(catalog, "c1000000-0000-4000-8000-000000000001")?.is_null(),
        "the root has no selected non-self parent"
    );
    Ok(())
}

fn assert_other_group_texts(catalog: &Value) -> TestResult {
    let mineral = groups::definition(catalog, "c2000000-0000-4000-8000-000000000002")?;
    commodity::assert_text(
        catalog,
        at(mineral, "/name")?,
        TextDomain::PublicName,
        Some("@LOC_PLACEHOLDER"),
        None,
        None,
    )?;
    let alloy = groups::definition(catalog, "c3000000-0000-4000-8000-000000000003")?;
    assert_eq!(
        groups::parent_guid(catalog, "c3000000-0000-4000-8000-000000000003")?,
        "c1000000-0000-4000-8000-000000000001",
        "Metal is the selected Alloy parent"
    );
    commodity::assert_text(
        catalog,
        at(alloy, "/name")?,
        TextDomain::PublicName,
        Some("@items_commodities_type_alloy"),
        None,
        None,
    )?;
    Ok(())
}
