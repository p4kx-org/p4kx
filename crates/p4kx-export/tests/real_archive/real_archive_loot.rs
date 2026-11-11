// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loot tables, archetypes, pool filters and the containers placed with a multi config in the
//! Basetool catalog of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{Archive, Guid};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn built() -> Result<(Value, Vec<String>), Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let warnings = built
        .summary()
        .warnings()
        .iter()
        .filter(|warning| warning.kind().contains("loot"))
        .map(|warning| format!("{warning}"))
        .collect();
    Ok((serde_json::from_slice(built.json())?, warnings))
}

fn catalog() -> Result<Value, Box<dyn Error>> {
    Ok(built()?.0)
}

fn list<'c>(catalog: &'c Value, kind: &str) -> Result<&'c Vec<Value>, String> {
    catalog[kind]
        .as_array()
        .ok_or_else(|| format!("the {kind} array"))
}

fn guids<'c>(catalog: &'c Value, kind: &str) -> Result<HashSet<&'c str>, Box<dyn Error>> {
    Ok(list(catalog, kind)?
        .iter()
        .filter_map(|entry| entry["guid"].as_str())
        .collect())
}

fn filter_guids(catalog: &Value) -> Result<HashSet<&str>, Box<dyn Error>> {
    let table = catalog
        .pointer("/definitions/lootPoolFilters")
        .ok_or("canonical Loot definitions table")?;
    assert_eq!(
        table.get("availability").and_then(Value::as_str),
        Some("available"),
        "the real Archive declares its Loot family"
    );
    let rows = table
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("canonical physical Loot rows")?;
    let mut guids = HashSet::new();
    for row in rows {
        let cell = row
            .get("guid")
            .ok_or("independently sourced original header GUID")?;
        assert_eq!(
            cell.get("state").and_then(Value::as_str),
            Some("present"),
            "an emitted original GUID is readable"
        );
        for member in [
            "containsEncryptedSource",
            "sourceUsesRefs",
            "sourceSetComplete",
        ] {
            assert!(
                cell.get(member).is_some(),
                "required immediate origin member {member}"
            );
        }
        let guid = cell
            .get("value")
            .and_then(Value::as_str)
            .ok_or("exact original GUID text")?;
        let _: Guid = guid.parse()?;
        guids.insert(guid);
    }
    Ok(guids)
}

fn named<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, Box<dyn Error>> {
    list(catalog, kind)?
        .iter()
        .find(|entry| entry["className"].as_str() == Some(class))
        .ok_or_else(|| format!("{kind} has no {class}").into())
}

fn array<'c>(entry: &'c Value, field: &str) -> &'c [Value] {
    entry[field].as_array().map_or(&[], Vec::as_slice)
}

/// Asserts that every GUID `values` yields is one of `known`.
fn assert_resolve<'c>(
    what: &str,
    values: impl IntoIterator<Item = &'c str>,
    known: &HashSet<&str>,
) {
    let missing: Vec<&str> = values
        .into_iter()
        .filter(|guid| !known.contains(guid))
        .take(5)
        .collect();
    assert!(missing.is_empty(), "{what} resolve: {missing:?}");
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_loot_table_of_a_real_archive_keeps_its_weights_and_links() -> TestResult {
    let catalog = catalog()?;
    let archetypes = guids(&catalog, "lootArchetypes")?;
    let table = named(&catalog, "lootTables", "V3LootTable_Actor_CQC_CouncilScrip")?;
    let entries = array(table, "entries");
    assert_eq!(entries.len(), 3, "the entries of the table");
    let ammo = &entries[0];
    assert_eq!(ammo["name"], "Ammo");
    assert_eq!(ammo["weight"], 10.0);
    assert!(
        ammo["archetype"].is_array(),
        "an archetype written out in place"
    );
    assert!(
        !array(ammo, "archetype").is_empty() && ammo["archetypeGuid"].is_null(),
        "the in-place archetype has entries and no link"
    );
    let medpens = &entries[1];
    assert_eq!(medpens["name"], "Medpens");
    assert!(medpens["archetype"].is_null());
    assert_resolve(
        "the linked archetype",
        medpens["archetypeGuid"].as_str(),
        &archetypes,
    );
    let unique = &entries[2];
    assert_eq!(unique["choiceLimit"], 1);
    assert_eq!(unique["priorityCount"], 1);
    assert_eq!(unique["chanceToExist"], 0.75);
    let selector = &array(unique, "archetype")[0]["selector"];
    assert_eq!(array(selector, "entityClassGuids").len(), 1);
    assert_eq!(array(&array(unique, "archetype")[0], "spawnWith").len(), 0);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_loot_kinds_and_the_containers_of_a_real_archive_are_linked() -> TestResult {
    let catalog = catalog()?;
    let tables = guids(&catalog, "lootTables")?;
    let archetypes = guids(&catalog, "lootArchetypes")?;
    let filters = filter_guids(&catalog)?;
    let configs = guids(&catalog, "subHarvestableMultiConfigs")?;
    let containers = list(&catalog, "lootContainers")?;
    print_line(&format!(
        "lootTables {}, lootArchetypes {}, lootPoolFilters {}, lootContainers {}",
        tables.len(),
        archetypes.len(),
        filters.len(),
        containers.len()
    ));
    assert!(tables.len() >= 50 && archetypes.len() >= 30 && filters.len() >= 10);
    for table in list(&catalog, "lootTables")? {
        let entries = array(table, "entries");
        assert!(!entries.is_empty(), "a table without entries");
        assert!(entries.iter().all(|entry| entry["weight"].is_number()));
        assert_resolve(
            "the archetypes of a table",
            entries
                .iter()
                .filter_map(|entry| entry["archetypeGuid"].as_str()),
            &archetypes,
        );
    }
    let mut linked = 0_usize;
    for config in list(&catalog, "subHarvestableMultiConfigs")? {
        for tagged in array(config, "taggedConfigs") {
            for slot in array(tagged, "subHarvestables") {
                let loot = &slot["lootConfig"];
                if loot.is_null() {
                    continue;
                }
                linked += 1;
                assert_resolve(
                    "the table of a slot",
                    loot["lootTableV3Guid"].as_str(),
                    &tables,
                );
                assert_resolve(
                    "the filter of a slot",
                    loot["poolFilterGuid"].as_str(),
                    &filters,
                );
            }
        }
    }
    print_line(&format!("slots with a loot config: {linked}"));
    assert!(linked > 0);
    assert!(!containers.is_empty(), "no loot container is placed");
    assert_resolve(
        "the config of a container",
        containers
            .iter()
            .filter_map(|row| row["multiConfigGuid"].as_str()),
        &configs,
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_older_loot_records_of_a_real_archive_resolve_and_the_scan_warns_of_nothing() -> TestResult {
    let (catalog, warnings) = built()?;
    for warning in &warnings {
        print_line(warning);
    }
    let tables = guids(&catalog, "legacyLootTables")?;
    let archetypes = guids(&catalog, "legacyLootArchetypes")?;
    print_line(&format!(
        "legacyLootTables {}, legacyLootArchetypes {}",
        tables.len(),
        archetypes.len()
    ));
    for table in list(&catalog, "legacyLootTables")? {
        assert_resolve(
            "the archetypes of an older table",
            array(table, "entries")
                .iter()
                .filter_map(|entry| entry["archetypeGuid"].as_str()),
            &archetypes,
        );
    }
    for config in list(&catalog, "subHarvestableMultiConfigs")? {
        for tagged in array(config, "taggedConfigs") {
            for slot in array(tagged, "subHarvestables") {
                assert_resolve(
                    "the older table of a slot",
                    slot["lootConfig"]["lootTableGuid"].as_str(),
                    &tables,
                );
            }
        }
    }
    Ok(())
}
