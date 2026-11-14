// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The older loot Records and the warnings of the loot container scan, in the Basetool catalog.

#[path = "loot_generation_component_tests.rs"]
mod loot_generation_component_tests;

use crate::catalog_loot::{build, entry};
use crate::loot_fixture::{
    GHOST, LEGACY_ARCHETYPE, LEGACY_TABLE, NESTED, TABLE, TAG_COMMON, TAG_RARE,
};
use crate::starmap_fixture::guid_text;

use std::error::Error;

use p4kx_export::Kind;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn an_older_loot_table_keeps_its_weights_and_result_limits() -> TestResult {
    let (built, catalog) = build()?;
    let table = entry(&catalog, "legacyLootTables", LEGACY_TABLE)?;
    assert_eq!(table["className"], "Cargo");
    assert_eq!(
        table["recordPath"],
        "libs/foundry/records/lootgeneration/loottables/cargo"
    );
    assert_eq!(
        table["entries"],
        json!([
            {
                "weight": 2.0,
                "archetypeGuid": guid_text(LEGACY_ARCHETYPE),
                "minResults": 0,
                "maxResults": 3
            },
            {"weight": 1.0, "archetypeGuid": null, "minResults": 0, "maxResults": 0}
        ])
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::LegacyLootTables)
            .map(|kind| (kind.count, kind.skipped)),
        Some((1, 0))
    );
    Ok(())
}

#[test]
fn an_older_loot_archetype_keeps_its_tags_optional_data_and_groups() -> TestResult {
    let (_, catalog) = build()?;
    let archetype = entry(&catalog, "legacyLootArchetypes", LEGACY_ARCHETYPE)?;
    assert_eq!(archetype["className"], "LootArchetype_Cargo");
    assert_eq!(archetype["excludedTagGuids"], json!([guid_text(TAG_RARE)]));
    assert_eq!(
        archetype["entries"],
        json!([{
            "name": "Cargo",
            "weight": 1.0,
            "tagGuid": guid_text(TAG_COMMON),
            "additionalPositiveTagGuids": [guid_text(TAG_RARE)],
            "additionalNegativeTagGuids": [],
            "stackSize": {"min": 3.0, "max": 5.0},
            "spawnWith": [{
                "name": "Suitable ammunition",
                "mode": "MostSimilar",
                "min": 1.0,
                "max": 3.0,
                "positiveTagGuids": [guid_text(TAG_COMMON)],
                "negativeTagGuids": [guid_text(TAG_RARE)]
            }]
        }])
    );
    assert_eq!(
        archetype["secondaryGroups"],
        json!([{
            "groupName": "Rarity",
            "entries": [{"weight": 4.0, "tagGuid": guid_text(TAG_RARE)}]
        }])
    );
    Ok(())
}

#[test]
fn an_older_record_is_no_v3_record_and_the_other_way_round() -> TestResult {
    let (_, catalog) = build()?;
    assert!(entry(&catalog, "lootTables", LEGACY_TABLE).is_err());
    assert!(entry(&catalog, "legacyLootTables", TABLE).is_err());
    Ok(())
}

fn warning_details(
    built: &p4kx_export::BuiltCatalog,
    kind: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    built
        .summary()
        .warnings()
        .iter()
        .find(|warning| warning.kind() == kind)
        .map(|warning| warning.details().to_vec())
        .ok_or_else(|| format!("no {kind} warning").into())
}

#[test]
fn a_placed_container_the_archive_lacks_is_warned_about() -> TestResult {
    let (built, catalog) = build()?;
    let details = warning_details(&built, "unreadable_loot_containers")?;
    assert_eq!(
        details,
        [format!(
            "{GHOST}: placed by an entity but not in the Archive"
        )]
    );
    let ghost = catalog["lootContainers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| row["container"] == GHOST)
        .ok_or("the missing container has a row")?;
    assert_eq!(ghost["entityCount"], Value::Null);
    Ok(())
}

#[test]
fn an_entity_file_with_several_containers_is_warned_about() -> TestResult {
    let (built, _) = build()?;
    let details = warning_details(&built, "ambiguous_loot_entity_files")?;
    assert_eq!(
        details,
        [format!("{NESTED}: entdata\\1.entxml")],
        "only the entity file with two different containers"
    );
    Ok(())
}
