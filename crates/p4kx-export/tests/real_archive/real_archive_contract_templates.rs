// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The contract templates in the Basetool catalog of a real Archive.

use std::collections::{BTreeMap, HashSet};
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_verification::{TestResult, guids, print_line};

/// Asserts that every `templateGuid` of a contract and every link of a template names an entry of
/// its kind and that every objective names a parent among those of its template.
pub(crate) fn check_template_links(catalog: &Value) -> TestResult {
    let templates = guids(catalog, "contractTemplates")?;
    for contract in catalog["contracts"].as_array().ok_or("contracts")? {
        if let Some(guid) = contract["templateGuid"].as_str() {
            assert!(templates.contains(guid), "template {guid} of a contract");
        }
    }
    let pools = guids(catalog, "contractShipSpawns")?;
    let queries = guids(catalog, "contractLocationQueries")?;
    let types = guids(catalog, "missionTypes")?;
    for template in catalog["contractTemplates"].as_array().ok_or("templates")? {
        let name = template["className"].as_str().unwrap_or_default();
        if let Some(guid) = template["missionTypeGuid"].as_str() {
            assert!(types.contains(guid), "{name}: mission type {guid}");
        }
        for link in template["shipSpawns"].as_array().ok_or("shipSpawns")? {
            let guid = link["poolGuid"].as_str().ok_or("poolGuid")?;
            assert!(pools.contains(guid), "{name}: ship spawn pool {guid}");
        }
        for link in template["locationQueries"]
            .as_array()
            .ok_or("locationQueries")?
        {
            let guid = link["queryGuid"].as_str().ok_or("queryGuid")?;
            assert!(queries.contains(guid), "{name}: location query {guid}");
        }
        let objectives = template["objectives"].as_array().ok_or("objectives")?;
        let ids: HashSet<&str> = objectives
            .iter()
            .filter_map(|objective| objective["id"].as_str())
            .collect();
        for objective in objectives {
            if let Some(parent) = objective["parentId"].as_str() {
                assert!(ids.contains(parent), "{name}: objective parent {parent}");
            }
            for property in objective["objectiveProperties"]
                .as_array()
                .ok_or("properties")?
            {
                check_property(name, property, (&ids, &pools, &queries));
            }
        }
    }
    Ok(())
}

/// Asserts that a property of an objective is of a known kind and that its links name an
/// objective of the template or an entry of the shared kinds.
fn check_property(
    name: &str,
    property: &Value,
    (ids, pools, queries): (&HashSet<&str>, &HashSet<&str>, &HashSet<&str>),
) {
    assert!(
        property["kind"].is_string(),
        "{name}: a property of an unknown kind"
    );
    let linked = property.pointer("/linkedOutput/objectiveId");
    if let Some(objective) = linked.and_then(Value::as_str) {
        assert!(
            ids.contains(objective),
            "{name}: output of objective {objective}"
        );
    }
    let value = &property["value"];
    if let Some(guid) = value["poolGuid"].as_str() {
        assert!(pools.contains(guid), "{name}: ship spawn pool {guid}");
    }
    if let Some(guid) = value["queryGuid"].as_str() {
        assert!(queries.contains(guid), "{name}: location query {guid}");
    }
}

/// Asserts that every contract-level property of a template and of a broker entry with a value
/// struct states a value whose links name an entry of the shared kinds, and returns how many
/// properties state each type of value.
fn check_contract_properties(catalog: &Value) -> Result<BTreeMap<String, usize>, String> {
    let mut pools = HashSet::new();
    for kind in [
        "contractShipSpawns",
        "contractNpcSpawns",
        "contractEntitySpawns",
    ] {
        pools.extend(guids(catalog, kind)?);
    }
    let queries = guids(catalog, "contractLocationQueries")?;
    let mut types = BTreeMap::new();
    for (kind, field) in [
        ("contractTemplates", "contractProperties"),
        ("missionBrokerEntries", "missionProperties"),
    ] {
        for entry in catalog[kind].as_array().ok_or(kind)? {
            for property in entry[field].as_array().ok_or(field)? {
                let value = &property["value"];
                if property["valueType"].is_null() {
                    assert!(value.is_null(), "a value without a value type");
                    continue;
                }
                let name = value["type"].as_str().ok_or("a value without a type")?;
                if let Some(guid) = value["poolGuid"].as_str() {
                    assert!(pools.contains(guid), "{kind}: spawn pool {guid}");
                }
                if let Some(guid) = value["queryGuid"].as_str() {
                    assert!(queries.contains(guid), "{kind}: location query {guid}");
                }
                *types.entry(name.to_owned()).or_default() += 1;
            }
        }
    }
    Ok(types)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_contract_properties_of_a_real_archive_state_their_values() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let failed: Vec<&str> = built
        .summary()
        .failures()
        .iter()
        .filter(|failure| failure.reason.contains("MissionPropertyValue_"))
        .map(|failure| failure.record_path.as_str())
        .collect();
    assert!(failed.is_empty(), "{failed:?}");
    let catalog: Value = serde_json::from_slice(built.json())?;
    let types = check_contract_properties(&catalog)?;
    print_line(&format!("contract property values by type: {types:?}"));
    for name in [
        "missionItem",
        "aiName",
        "combinedDataSetEntries",
        "haulingOrders",
        "reward",
        "timeTrialRace",
        "starMapLocation",
        "location",
        "shipSpawnDescriptions",
    ] {
        assert!(types.contains_key(name), "no property of the type {name}");
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_contract_templates_of_a_real_archive_resolve() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_template_links(&catalog)?;
    let templates = catalog["contractTemplates"].as_array().ok_or("templates")?;
    let used: HashSet<&str> = catalog["contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|contract| contract["templateGuid"].as_str())
        .collect();
    let count = |field: &str| -> usize {
        templates
            .iter()
            .map(|template| template[field].as_array().map_or(0, Vec::len))
            .sum()
    };
    let (objectives, ships, queries, hauling) = (
        count("objectives"),
        count("shipSpawns"),
        count("locationQueries"),
        count("haulingOrders"),
    );
    print_line(&format!(
        "{} contract templates, {} named by a contract, {objectives} objectives, {ships} ship spawn links, {queries} location query links, {hauling} hauling orders",
        templates.len(),
        used.len(),
    ));
    assert!(templates.len() > 400, "templates");
    assert!(objectives > 500, "objectives");
    assert!(ships > 0 && queries > 0 && hauling > 0, "links and orders");
    Ok(())
}
