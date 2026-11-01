// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The NPC and entity spawn descriptions of contracts in the Basetool catalog of a real Archive.

use std::collections::{BTreeMap, HashSet};

use serde_json::Value;

use crate::real_archive_verification::{TestResult, guids, print_line};

/// The values of the enum `EExcludeSpawnGender`.
const GENDERS: [&str; 3] = ["Male", "Female", "None"];

/// The `type` of every value an objective property holds.
const VALUE_TYPES: [&str; 11] = [
    "object",
    "deliveryOrder",
    "boolean",
    "stringHash",
    "integer",
    "float",
    "tags",
    "shipSpawnDescriptions",
    "location",
    "npcSpawnDescriptions",
    "entitySpawnDescriptions",
];

/// The three fields that hold links to the spawn kinds, with the kind each one links to.
const LINKS: [(&str, &str); 3] = [
    ("shipSpawns", "contractShipSpawns"),
    ("npcSpawns", "contractNpcSpawns"),
    ("entitySpawns", "contractEntitySpawns"),
];

/// What the links of one Archive amount to.
#[derive(Debug, Default)]
struct Counts {
    links: BTreeMap<&'static str, usize>,
    property_values: usize,
    organizations: usize,
}

fn check_links_of(
    entry: &Value,
    kinds: &[(&'static str, HashSet<&str>)],
    counts: &mut Counts,
) -> TestResult {
    for (field, listed) in kinds {
        for link in entry[*field].as_array().ok_or(*field)? {
            let guid = link["poolGuid"].as_str().ok_or("poolGuid")?;
            assert!(listed.contains(guid), "{field} {guid} resolves");
            *counts.links.entry(field).or_default() += 1;
        }
    }
    for objective in entry["objectives"].as_array().into_iter().flatten() {
        for property in objective["objectiveProperties"]
            .as_array()
            .ok_or("properties")?
        {
            let value = &property["value"];
            if value.is_null() {
                continue;
            }
            let kind = value["type"].as_str().ok_or("value type")?;
            if kind == "organization" {
                counts.organizations += 1;
                continue;
            }
            assert!(VALUE_TYPES.contains(&kind), "known value type {kind}");
            let field = match kind {
                "shipSpawnDescriptions" => "shipSpawns",
                "npcSpawnDescriptions" => "npcSpawns",
                "entitySpawnDescriptions" => "entitySpawns",
                _ => continue,
            };
            if let Some(guid) = value["poolGuid"].as_str() {
                let listed = &kinds
                    .iter()
                    .find(|(name, _)| *name == field)
                    .ok_or("kind")?
                    .1;
                assert!(listed.contains(guid), "{kind} {guid} resolves");
            }
            counts.property_values += 1;
        }
    }
    Ok(())
}

/// Asserts that every NPC and entity spawn link of a contract, a template and a broker entry, and
/// every spawn value of an objective property, names an entry of its kind, and that the stored
/// enum names are known.
pub(crate) fn check_npc_links(catalog: &Value) -> TestResult {
    let kinds: Vec<(&'static str, HashSet<&str>)> = LINKS
        .into_iter()
        .map(|(field, kind)| Ok((field, guids(catalog, kind)?)))
        .collect::<Result<_, String>>()?;
    let mut counts = Counts::default();
    for kind in ["contracts", "contractTemplates", "missionBrokerEntries"] {
        for entry in catalog[kind].as_array().ok_or(kind)? {
            check_links_of(entry, &kinds, &mut counts)?;
        }
    }
    let mut unknown_genders = 0_usize;
    let mut options = 0_usize;
    for pool in catalog["contractNpcSpawns"].as_array().ok_or("npc pools")? {
        for group in pool["groups"].as_array().ok_or("groups")? {
            for option in group["options"].as_array().ok_or("options")? {
                options += 1;
                if let Some(gender) = option
                    .pointer("/autoSpawnSettings/excludeSpawnGender")
                    .and_then(Value::as_str)
                    && !GENDERS.contains(&gender)
                {
                    unknown_genders += 1;
                }
            }
        }
    }
    let entity_options: usize = catalog["contractEntitySpawns"]
        .as_array()
        .ok_or("entity pools")?
        .iter()
        .flat_map(|pool| pool["groups"].as_array().into_iter().flatten())
        .flat_map(|group| group["slots"].as_array().into_iter().flatten())
        .map(|slot| slot["options"].as_array().map_or(0, Vec::len))
        .sum();
    print_line(&format!(
        "{} NPC spawns with {options} options, {} entity spawns with {entity_options} options; \
         links {:?}, {} spawn values and {} organisations in objective properties",
        catalog["contractNpcSpawns"].as_array().map_or(0, Vec::len),
        catalog["contractEntitySpawns"]
            .as_array()
            .map_or(0, Vec::len),
        counts.links,
        counts.property_values,
        counts.organizations,
    ));
    let linked = |field: &str| counts.links.get(field).copied().unwrap_or_default();
    assert_eq!(unknown_genders, 0, "every excluded gender is a known one");
    assert!(options > 1_000, "NPC options: {options}");
    assert!(entity_options > 300, "entity options: {entity_options}");
    assert!(linked("npcSpawns") > 1_000, "NPC links: {counts:?}");
    assert!(linked("entitySpawns") > 200, "entity links: {counts:?}");
    Ok(())
}
