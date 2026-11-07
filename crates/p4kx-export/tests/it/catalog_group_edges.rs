// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Edge cases of resource groups and blueprint categories: leftovers, unreadable groups, cycles.

use crate::crafting_fixture;

use std::collections::HashSet;
use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const FIRST: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;
const SECOND: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0002;
const THIRD: u128 = 0x3000_0000_0000_4000_8000_0000_0000_0003;
const LEFTOVER_GROUP: u128 = 0x4000_0000_0000_4000_8000_0000_0000_0004;
const ORPHAN_BLUEPRINT: u128 = 0x1800_0000_0000_4000_8000_0000_0000_0008;
const GROUP_PATH: &str = "libs/foundry/records/resourcetypedatabase/resourcetypedatabase.xml";

fn group_struct() -> TestStruct {
    TestStruct::new("ResourceTypeGroup")
        .field("displayName", TestType::Locale)
        .array("groups", TestType::Reference)
        .array("resources", TestType::Reference)
}

fn group(guid: u128, class: &str, children: &[u128]) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceTypeGroup",
        format!("ResourceTypeGroup.{class}"),
        GROUP_PATH,
    )
    .value("displayName", TestValue::text("@LOC_PLACEHOLDER"))
    .value(
        "groups",
        TestValue::Array(
            children
                .iter()
                .map(|guid| TestValue::Reference(*guid))
                .collect(),
        ),
    )
    .value("resources", TestValue::Array(Vec::new()))
}

fn build_from(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_mode(forge, filter, p4kx_export::CatalogPublicationMode::Normal)
}

fn diagnostic_from(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_mode(
        forge,
        filter,
        p4kx_export::CatalogPublicationMode::DiagnosticPartial,
    )
}

fn build_mode(
    forge: &DataForgeBuilder,
    filter: Option<&Filter>,
    publication_mode: p4kx_export::CatalogPublicationMode,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()).encrypted())
        .entry(crafting_fixture::english())
        .entry(crafting_fixture::german())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?;
    let options = CatalogOptions {
        publication_mode,
        filter,
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

fn groups_only(records: Vec<TestRecord>) -> DataForgeBuilder {
    records.into_iter().fold(
        DataForgeBuilder::new().version(8).structure(group_struct()),
        DataForgeBuilder::record,
    )
}

fn parent_of(catalog: &Value, guid: &str) -> Option<Value> {
    catalog["resourceGroups"]
        .as_array()?
        .iter()
        .find(|group| group["guid"] == guid)
        .map(|group| group["parentGuid"].clone())
}

#[test]
fn a_group_that_lists_itself_has_no_parent() -> TestResult {
    let (_, catalog) = build_from(&groups_only(vec![group(FIRST, "Loop", &[FIRST])]), None)?;
    assert_eq!(
        parent_of(&catalog, "10000000-0000-4000-8000-000000000001"),
        Some(Value::Null),
        "a self-list is no parent"
    );
    Ok(())
}

#[test]
fn two_groups_that_list_each_other_are_each_others_parent_and_the_build_ends() -> TestResult {
    let (built, catalog) = build_from(
        &groups_only(vec![
            group(FIRST, "Ping", &[SECOND]),
            group(SECOND, "Pong", &[FIRST]),
            group(THIRD, "Root", &[]),
        ]),
        None,
    )?;
    assert_eq!(
        parent_of(&catalog, "10000000-0000-4000-8000-000000000001"),
        Some(json!("20000000-0000-4000-8000-000000000002"))
    );
    assert_eq!(
        parent_of(&catalog, "20000000-0000-4000-8000-000000000002"),
        Some(json!("10000000-0000-4000-8000-000000000001"))
    );
    assert_eq!(
        parent_of(&catalog, "30000000-0000-4000-8000-000000000003"),
        Some(Value::Null)
    );
    assert!(built.summary().is_complete(), "a cycle is no failure");
    Ok(())
}

#[test]
fn a_leftover_group_names_no_parent_and_every_parent_resolves() -> TestResult {
    let (_, catalog) = build_from(
        &groups_only(vec![
            group(FIRST, "Metal", &[SECOND]),
            group(SECOND, "Alloy", &[]),
            group(LEFTOVER_GROUP, "Test_Group", &[THIRD]),
            group(THIRD, "Orphan", &[]),
        ]),
        None,
    )?;
    let guids: HashSet<&str> = catalog["resourceGroups"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|group| group["guid"].as_str())
        .collect();
    assert_eq!(guids.len(), 3, "the leftover group is left out");
    assert_eq!(
        parent_of(&catalog, "30000000-0000-4000-8000-000000000003"),
        Some(Value::Null),
        "a leftover lists it, so it names no parent"
    );
    for group in catalog["resourceGroups"].as_array().into_iter().flatten() {
        if let Some(parent) = group["parentGuid"].as_str() {
            assert!(guids.contains(parent), "parent {parent}");
        }
    }
    Ok(())
}

fn unreadable_groups() -> DataForgeBuilder {
    let broken = TestRecord::new(
        FIRST,
        "ResourceTypeGroup",
        "ResourceTypeGroup.Broken",
        GROUP_PATH,
    );
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ResourceTypeGroup")
                .field("a", TestType::Int8)
                .declared_size(3),
        )
        .record(broken)
}

#[test]
fn an_unreadable_group_is_reported_once() -> TestResult {
    let (built, _) = diagnostic_from(&unreadable_groups(), None)?;
    let failures: Vec<String> = built
        .summary()
        .failures()
        .iter()
        .map(|failure| failure.guid.to_string())
        .collect();
    assert_eq!(failures, ["10000000-0000-4000-8000-000000000001"]);
    Ok(())
}

#[test]
fn an_unreadable_group_outside_the_filter_is_still_reported_once() -> TestResult {
    let filter = Filter::new("*/blueprintcategories/*");
    let (built, _) = diagnostic_from(&unreadable_groups(), Some(&filter))?;
    assert_eq!(
        built.summary().failures().len(),
        1,
        "the map needs the group, so the failure stays"
    );
    Ok(())
}

#[test]
fn a_blueprint_whose_category_is_unreadable_has_no_category_guid() -> TestResult {
    let pointer = |name: &str| TestType::StrongPointer(name.to_owned());
    let forge = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("BlueprintCategoryRecord")
                .field("a", TestType::Int8)
                .declared_size(3),
        )
        .structure(
            TestStruct::new("CraftingBlueprintRecord")
                .field("blueprint", pointer("CraftingBlueprintBase")),
        )
        .structure(TestStruct::new("CraftingBlueprintBase"))
        .structure(
            TestStruct::new("CraftingBlueprint")
                .parent("CraftingBlueprintBase")
                .field("category", TestType::Reference),
        )
        .record(TestRecord::new(
            FIRST,
            "BlueprintCategoryRecord",
            "BlueprintCategoryRecord.Broken",
            "libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase.xml",
        ))
        .record(
            TestRecord::new(
                SECOND,
                "CraftingBlueprintRecord",
                "CraftingBlueprintRecord.BP_CRAFT_broken_category",
                "libs/foundry/records/crafting/blueprints/crafting/bp_craft_broken_category.xml",
            )
            .value(
                "blueprint",
                TestValue::Struct(
                    TestInstance::new("CraftingBlueprint")
                        .value("category", TestValue::Reference(FIRST)),
                ),
            ),
        );
    let (built, catalog) = diagnostic_from(&forge, None)?;
    assert_eq!(catalog["blueprints"][0]["categoryGuid"], Value::Null);
    assert_eq!(
        built.summary().failures().len(),
        1,
        "the category is listed"
    );
    Ok(())
}

#[test]
fn a_resource_type_listed_only_by_a_leftover_group_gives_its_commodity_no_type_guid() -> TestResult
{
    const RESOURCE: u128 = 0x5000_0000_0000_4000_8000_0000_0000_0005;
    let resource = TestRecord::new(
        RESOURCE,
        "ResourceType",
        "ResourceType.zinc",
        "libs/foundry/records/resourcetypedatabase/resourcetypedatabase.xml",
    )
    .value("displayName", TestValue::text("@items_commodities_zinc"));
    let commodity = TestRecord::new(
        0x6000_0000_0000_4000_8000_0000_0000_0006,
        "EntityClassDefinition",
        "EntityClassDefinition.zinc",
        "libs/foundry/records/entities/commodities/metals/zinc.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("ResourceContainer").value(
                "defaultComposition",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("ResourceContainerDefaultCompositionEntry")
                        .value("entry", TestValue::Reference(RESOURCE)),
                )]),
            ),
        )]),
    );
    let leftover = TestRecord::new(
        LEFTOVER_GROUP,
        "ResourceTypeGroup",
        "ResourceTypeGroup.Test_Group",
        GROUP_PATH,
    )
    .value("displayName", TestValue::text("@LOC_PLACEHOLDER"))
    .value("groups", TestValue::Array(Vec::new()))
    .value(
        "resources",
        TestValue::Array(vec![TestValue::Reference(RESOURCE)]),
    );
    let forge = crafting_fixture::database()
        .record(resource)
        .record(commodity)
        .record(leftover);
    let (_, catalog) = build_from(&forge, None)?;
    let zinc = catalog["commodities"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|commodity| commodity["className"] == "zinc")
        .ok_or("the zinc commodity")?;
    assert_eq!(zinc["typeGuid"], Value::Null);
    assert!(
        zinc["subtypeGuid"].is_string(),
        "the ResourceType is still linked"
    );
    Ok(())
}

#[test]
fn a_blueprint_whose_category_is_a_leftover_has_no_category_guid() -> TestResult {
    let orphan = TestRecord::new(
        ORPHAN_BLUEPRINT,
        "CraftingBlueprintRecord",
        "CraftingBlueprintRecord.BP_CRAFT_orphan",
        "libs/foundry/records/crafting/blueprints/crafting/fpsgear/bp_craft_orphan.xml",
    )
    .value(
        "blueprint",
        TestValue::Struct(
            TestInstance::new("CraftingBlueprint")
                .value("blueprintName", TestValue::text("@LOC_PLACEHOLDER"))
                .value(
                    "category",
                    TestValue::Reference(crafting_fixture::CATEGORY_LEFTOVER),
                ),
        ),
    );
    let (_, catalog) = build_from(&crafting_fixture::database().record(orphan), None)?;
    let categories: HashSet<&str> = catalog["blueprintCategories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|category| category["guid"].as_str())
        .collect();
    let blueprints = catalog["blueprints"].as_array().ok_or("blueprints")?;
    let orphan = blueprints
        .iter()
        .find(|blueprint| blueprint["className"] == "BP_CRAFT_orphan")
        .ok_or("the orphan blueprint")?;
    assert_eq!(orphan["categoryGuid"], Value::Null);
    for blueprint in blueprints {
        if let Some(guid) = blueprint["categoryGuid"].as_str() {
            assert!(categories.contains(guid), "category {guid}");
        }
    }
    assert!(
        blueprints
            .iter()
            .any(|blueprint| blueprint["categoryGuid"].is_string()),
        "the others keep theirs"
    );
    Ok(())
}
