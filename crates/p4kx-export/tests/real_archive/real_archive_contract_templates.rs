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

#[path = "real_archive_mission_graph.rs"]
pub(crate) mod mission_graph;
use mission_graph::{self as graph, Family, Mission};

/// Checks the canonical Template contexts, their original links and objective parents.
pub(crate) fn check_template_links(catalog: &Value) -> TestResult {
    let templates = graph::contexts(catalog, Family::Template)?;
    let identities = templates
        .iter()
        .map(|mission| graph::guid(mission.definition))
        .collect::<Result<HashSet<_>, _>>()?;
    for contract in crate::real_archive_contracts::v3::rows(catalog)? {
        if let Some(guid) = contract["templateGuid"].as_str() {
            assert!(identities.contains(guid), "template {guid} of a contract");
        }
    }
    let pools = guids(catalog, "contractShipSpawns")?;
    let queries = guids(catalog, "contractLocationQueries")?;
    let types = guids(catalog, "missionTypes")?;
    for template in &templates {
        let name = graph::text(template.definition, "/className/value")?;
        if let Some(selected) = template
            .projection
            .pointer("/inputs/displayInfo/declaration/field")
            && let Some(info) = template.first_struct(template.field(selected)?)?
            && let Some(value) = template.member(info, "type")?
            && let Some(guid) = graph::reference(value)?
        {
            assert!(types.contains(guid), "{name}: mission type {guid}");
        }
        for link in template.pools()? {
            match link.family {
                "ships" => assert!(
                    pools.contains(link.guid),
                    "{name}: ship spawn pool {}",
                    link.guid
                ),
                "location" => assert!(
                    queries.contains(link.guid),
                    "{name}: location query {}",
                    link.guid
                ),
                _ => {}
            }
        }
        template.parents()?;
        objective_properties(template, name, (&pools, &queries))?;
    }
    Ok(())
}

fn objective_properties(
    mission: &Mission<'_>,
    name: &str,
    (pools, queries): (&HashSet<&str>, &HashSet<&str>),
) -> TestResult {
    let objectives = graph::array(mission.projection, "objectives")?;
    let ids = objectives
        .iter()
        .map(|objective| {
            objective
                .get("id")
                .ok_or("original objective id declaration")
        })
        .collect::<Result<Vec<_>, _>>()?;
    for property in graph::array(mission.projection, "properties")?
        .iter()
        .filter(|property| {
            property
                .get("objective")
                .is_some_and(|reference| !reference.is_null())
        })
    {
        assert!(
            property.get("kind").is_some_and(Value::is_string),
            "{name}: a property of an unknown kind"
        );
        let owner = property
            .get("objective")
            .ok_or("the actual objective owner")?;
        assert!(
            objectives
                .iter()
                .any(|objective| objective.get("source") == Some(owner)),
            "{name}: property belongs to its actual original objective"
        );
        let Some(source) = property.get("value").filter(|value| !value.is_null()) else {
            continue;
        };
        if let Some((family, guid)) = mission
            .registration(source)?
            .map(|result| result.identifier())
            .transpose()?
            .flatten()
        {
            match family {
                "ships" => assert!(pools.contains(guid), "{name}: ship spawn pool {guid}"),
                "location" => assert!(queries.contains(guid), "{name}: location query {guid}"),
                _ => {}
            }
        }
    }
    objective_outputs(mission, name, &ids)
}

fn objective_outputs(mission: &Mission<'_>, name: &str, ids: &[&Value]) -> TestResult {
    for output in graph::array(mission.projection, "linkedOutputs")? {
        if output
            .pointer("/objective/interpretation")
            .and_then(Value::as_str)
            == Some("id")
        {
            let target = output
                .pointer("/objective/field")
                .ok_or("actual target id declaration")?;
            mission.field(target)?;
            assert!(
                ids.contains(&target),
                "{name}: linked output targets an actual objective id"
            );
        }
    }
    Ok(())
}

/// Counts original contract-level property meanings and validates their selected registration.
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
    for family in [Family::Template, Family::Broker] {
        for mission in graph::contexts(catalog, family)? {
            for property in graph::array(mission.projection, "properties")?
                .iter()
                .filter(|property| property.get("objective") == Some(&Value::Null))
            {
                let value = property
                    .get("value")
                    .ok_or("required nullable original value")?;
                let kind = property
                    .get("valueKind")
                    .ok_or("required nullable property meaning")?;
                if kind.is_null() {
                    assert!(value.is_null(), "a value without a value type");
                    continue;
                }
                let name = kind.as_str().ok_or("a value without a type")?;
                assert!(
                    mission
                        .follow(value)?
                        .pointer("/value/value/type")
                        .and_then(Value::as_str)
                        == Some("struct"),
                    "a recognized value retains its actual original Struct"
                );
                if let Some((family, guid)) = mission
                    .registration(value)?
                    .map(|result| result.identifier())
                    .transpose()?
                    .flatten()
                {
                    if family == "location" {
                        assert!(queries.contains(guid), "location query {guid}");
                    } else {
                        assert!(pools.contains(guid), "spawn pool {guid}");
                    }
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
        .filter(|failure| failure.reason().contains("MissionPropertyValue_"))
        .map(|failure| failure.record_path())
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
    let templates = graph::contexts(&catalog, Family::Template)?;
    let used: HashSet<&str> = crate::real_archive_contracts::v3::rows(&catalog)?
        .iter()
        .filter_map(|contract| contract["templateGuid"].as_str())
        .collect();
    let (mut objectives, mut ships, mut queries, mut hauling) = (0, 0, 0, 0);
    for mission in &templates {
        objectives += graph::array(mission.projection, "objectives")?.len();
        hauling += graph::array(mission.projection, "haulingOrders")?.len();
        for link in mission.pools()? {
            ships += usize::from(link.family == "ships");
            queries += usize::from(link.family == "location");
        }
    }
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
