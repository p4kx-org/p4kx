// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The contract templates in the Basetool catalog of a real Archive.

use std::collections::HashSet;
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
