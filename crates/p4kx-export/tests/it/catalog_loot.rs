// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loot tables, archetypes, pool filters, the loot of a slot and the containers placed with a
//! multi config, in the Basetool catalog.

use crate::catalog_loot_filter_assertions;
use crate::loot_fixture::{
    ARCHETYPE, AREA, CONFIG, ENTITY, FILTER, FILTER_LEAF, GHOST, LEGACY_TABLE, NESTED, PLANET,
    TABLE, TAG_COMMON, TAG_RARE, write_archive,
};
use crate::loot_v3_graph::{self as graph, Graph};
use crate::starmap_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, CatalogPublicationMode, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

pub(crate) fn build() -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive =
        Archive::open_with_key(write_archive(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

pub(crate) fn entry<'a>(
    catalog: &'a Value,
    kind: &str,
    record: u128,
) -> Result<&'a Value, Box<dyn Error>> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == starmap_fixture::guid_text(record).as_str())
        .ok_or_else(|| format!("{kind} has no {}", starmap_fixture::guid_text(record)).into())
}

#[test]
fn a_loot_table_keeps_its_weights_and_the_optional_data_of_each_entry() -> TestResult {
    let (built, catalog) = build()?;
    let table = entry(&catalog, "lootTables", TABLE)?;
    assert_eq!(table["className"], "V3LootTable_Cargo");
    assert_eq!(
        table["recordPath"],
        "libs/foundry/records/lootgeneration/loottables/v3loottable_cargo"
    );
    let ammo = &table["entries"][0];
    assert_eq!(ammo["name"], "Ammo");
    assert_eq!(ammo["weight"], 10.0);
    assert_eq!(ammo["archetypeGuid"], Value::Null);
    assert_eq!(ammo["priorityCount"], 1);
    assert_eq!(ammo["choiceLimit"], 2);
    assert_eq!(ammo["dupeLimit"], 3);
    assert_eq!(ammo["chanceToExist"], 0.75);
    let linked = &table["entries"][1];
    assert_eq!(
        linked["archetypeGuid"],
        starmap_fixture::guid_text(ARCHETYPE).as_str()
    );
    assert_eq!(linked["archetype"], Value::Null);
    assert_eq!(
        [
            linked["priorityCount"].clone(),
            linked["choiceLimit"].clone(),
            linked["dupeLimit"].clone(),
            linked["chanceToExist"].clone()
        ],
        [Value::Null, Value::Null, Value::Null, Value::Null]
    );
    let kind = built.summary().kind(Kind::LootTables);
    assert_eq!(
        kind.map(|kind| (kind.count, kind.skipped)),
        Some((1, 1)),
        "the debug table is a leftover and the older LootTable is no V3 table"
    );
    Ok(())
}

#[test]
fn an_archetype_written_out_in_place_keeps_its_selector_stack_size_and_spawn_with() -> TestResult {
    let (_, catalog) = build()?;
    let table = entry(&catalog, "lootTables", TABLE)?;
    let selector = &table["entries"][0]["archetype"][0];
    assert_eq!(selector["name"], "Ammo");
    assert_eq!(selector["weight"], 3.0);
    assert_eq!(
        selector["selector"],
        json!({
            "selectorType": "LootArchetypeV3Selector_Tags",
            "positiveTagGuids": [starmap_fixture::guid_text(TAG_COMMON), starmap_fixture::guid_text(TAG_RARE)],
            "negativeTagGuids": [starmap_fixture::guid_text(TAG_RARE)],
            "entityClassGuids": []
        })
    );
    assert_eq!(selector["stackSize"], json!({"min": 1.0, "max": 3.0}));
    assert_eq!(
        selector["spawnWith"],
        json!([{
            "name": "Suitable ammo",
            "chance": 1.0,
            "mode": "MostSimilar",
            "amount": {"min": 2.0, "max": 4.0},
            "selector": {
                "selectorType": "SpawnWithV3Selector_Tags",
                "positiveTagGuids": [starmap_fixture::guid_text(TAG_COMMON)],
                "negativeTagGuids": [],
                "entityClassGuids": []
            }
        }])
    );
    Ok(())
}

#[test]
fn an_archetype_selects_entity_classes_without_tags() -> TestResult {
    let (_, catalog) = build()?;
    let archetype = entry(&catalog, "lootArchetypes", ARCHETYPE)?;
    assert_eq!(archetype["className"], "V3LootArchetype_Cargo");
    assert_eq!(
        archetype["entries"],
        json!([{
            "name": "Unique",
            "weight": 1.0,
            "selector": {
                "selectorType": "LootArchetypeV3Selector_EntityClasses",
                "positiveTagGuids": [],
                "negativeTagGuids": [],
                "entityClassGuids": [starmap_fixture::guid_text(ENTITY)]
            },
            "stackSize": null,
            "spawnWith": []
        }])
    );
    Ok(())
}

#[test]
fn a_pool_filter_nests_a_link_and_tag_terms() -> TestResult {
    let (_, catalog) = build()?;
    let leaf = Graph::new(&catalog, &starmap_fixture::guid_text(FILTER_LEAF))?;
    let fields = graph::structure(leaf.root("filter")?, "PoolFilter_Tags")?;
    catalog_loot_filter_assertions::tags(
        &leaf,
        leaf.field(fields, "tagFilter")?,
        &[TAG_COMMON],
        &[],
    )?;
    let filter = Graph::new(&catalog, &starmap_fixture::guid_text(FILTER))?;
    let fields = graph::structure(filter.root("filter")?, "PoolFilter_Sequence")?;
    assert_eq!(fields.len(), 1, "the actual sequence declares only filters");
    let slots = graph::elements(filter.field(fields, "filters")?)?;
    assert_eq!(
        slots.len(),
        3,
        "Base, Event and Empty retain original order"
    );
    let base = catalog_loot_filter_assertions::step(&filter, slots, 0, "Base", "Cumulative")?;
    let fields = graph::structure(base, "PoolFilter_RecordRef")?;
    assert_eq!(
        fields.len(),
        1,
        "the link has no invented tag terms or owned children"
    );
    assert_eq!(
        graph::reference_guid(filter.field(fields, "filterRecord")?)?,
        starmap_fixture::guid_text(FILTER_LEAF),
        "Base links the original leaf"
    );
    let event = catalog_loot_filter_assertions::step(&filter, slots, 1, "Event", "Additive")?;
    let fields = graph::structure(event, "PoolFilter_Tags")?;
    catalog_loot_filter_assertions::tags(
        &filter,
        filter.field(fields, "tagFilter")?,
        &[],
        &[TAG_RARE],
    )?;
    graph::source_null(catalog_loot_filter_assertions::step(
        &filter, slots, 2, "Empty", "Additive",
    )?);
    Ok(())
}

#[test]
fn a_slot_names_the_tables_and_the_constraints_of_its_loot() -> TestResult {
    let (_, catalog) = build()?;
    let config = entry(&catalog, "subHarvestableMultiConfigs", CONFIG)?;
    let slots = &config["taggedConfigs"][0]["subHarvestables"];
    assert_eq!(
        slots[0]["lootConfig"],
        json!({
            "lootTableGuid": starmap_fixture::guid_text(LEGACY_TABLE),
            "lootTableV3Guid": starmap_fixture::guid_text(TABLE),
            "poolFilterGuid": starmap_fixture::guid_text(FILTER),
            "totalResultsLimit": 4,
            "chanceToGenerate": 1.0,
            "chanceToGenerateAdditionalInventories": 0.5,
            "fullness": {"min": 0.2, "max": 0.8}
        })
    );
    assert_eq!(slots.as_array().map(Vec::len), Some(1));
    Ok(())
}

#[test]
fn a_container_placed_with_a_config_is_listed_for_each_root_it_hangs_below() -> TestResult {
    let (built, catalog) = build()?;
    let area = |object: Value, entities: Value, tags: Value| {
        json!({
            "multiConfigGuid": starmap_fixture::guid_text(CONFIG),
            "container": AREA,
            "system": "stanton",
            "starmapObjectGuid": object,
            "entityCount": entities,
            "tagCounts": tags
        })
    };
    let nested = |object: Value| {
        json!({
            "multiConfigGuid": starmap_fixture::guid_text(CONFIG),
            "container": NESTED,
            "system": "stanton",
            "starmapObjectGuid": object,
            "entityCount": 0,
            "tagCounts": []
        })
    };
    let tags = json!([
        {"tagGuid": starmap_fixture::guid_text(TAG_RARE), "entities": 2},
        {"tagGuid": starmap_fixture::guid_text(TAG_COMMON), "entities": 3}
    ]);
    let planet = json!(starmap_fixture::guid_text(PLANET));
    assert_eq!(
        catalog["lootContainers"],
        json!([
            area(Value::Null, json!(5), tags.clone()),
            area(planet.clone(), json!(5), tags),
            nested(Value::Null),
            nested(planet.clone()),
            {
                "multiConfigGuid": starmap_fixture::guid_text(CONFIG),
                "container": GHOST,
                "system": "stanton",
                "starmapObjectGuid": planet,
                "entityCount": null,
                "tagCounts": []
            }
        ])
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::LootContainers)
            .map(|kind| kind.count),
        Some(5)
    );
    Ok(())
}
