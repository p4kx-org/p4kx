// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The mission broker entries in the Basetool catalog of a real Archive.

use std::collections::{BTreeMap, HashSet};
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_verification::{TestResult, guids, print_line};

/// The fields of an entry that hold one GUID and the kind they link to.
const LINKS: [(&str, &str); 5] = [
    ("missionTypeGuid", "missionTypes"),
    ("locationGuid", "starmapObjects"),
    ("localityGuid", "missionLocalities"),
    ("invitationMissionGuid", "missionBrokerEntries"),
    ("linkedMissionGuid", "missionBrokerEntries"),
];

/// Asserts that every `brokerEntryGuid` of a contract names an entry, that the links of an entry
/// resolve to the entry of their kind when they name one the catalog lists, and that every
/// objective names a parent among those of its entry. Returns the links that name a Record the
/// catalog does not list, per field.
pub(crate) fn check_broker_links(catalog: &Value) -> Result<BTreeMap<&'static str, usize>, String> {
    let entries = guids(catalog, "missionBrokerEntries")?;
    for contract in catalog["contracts"].as_array().ok_or("contracts")? {
        if let Some(guid) = contract["brokerEntryGuid"].as_str()
            && !entries.contains(guid)
        {
            return Err(format!("broker entry {guid} of a contract"));
        }
    }
    let pools = guids(catalog, "contractShipSpawns")?;
    let queries = guids(catalog, "contractLocationQueries")?;
    let mut unlisted = BTreeMap::new();
    let targets = LINKS
        .into_iter()
        .map(|(field, kind)| Ok((field, guids(catalog, kind)?)))
        .collect::<Result<Vec<_>, String>>()?;
    for entry in catalog["missionBrokerEntries"]
        .as_array()
        .ok_or("entries")?
    {
        let name = entry["className"].as_str().unwrap_or_default();
        for (field, listed) in &targets {
            if let Some(guid) = entry[*field].as_str()
                && !listed.contains(guid)
            {
                *unlisted.entry(*field).or_insert(0) += 1;
            }
        }
        for field in [
            "associatedMissionGuids",
            "requiredMissionGuids",
            "unlessAvailableMissionGuids",
        ] {
            for guid in entry[field].as_array().ok_or(field)? {
                let guid = guid.as_str().ok_or(field)?;
                if !entries.contains(guid) {
                    *unlisted.entry("missionGuids").or_insert(0) += 1;
                }
            }
        }
        check_parts(name, entry, (&pools, &queries, &entries))?;
    }
    Ok(unlisted)
}

fn check_parts(
    name: &str,
    entry: &Value,
    (pools, queries, entries): (&HashSet<&str>, &HashSet<&str>, &HashSet<&str>),
) -> Result<(), String> {
    for link in entry["shipSpawns"].as_array().ok_or("shipSpawns")? {
        let guid = link["poolGuid"].as_str().ok_or("poolGuid")?;
        if !pools.contains(guid) {
            return Err(format!("{name}: ship spawn pool {guid}"));
        }
        if link["level"] != "broker" {
            return Err(format!("{name}: level of a ship spawn"));
        }
    }
    for link in entry["locationQueries"].as_array().ok_or("queries")? {
        let guid = link["queryGuid"].as_str().ok_or("queryGuid")?;
        if !queries.contains(guid) {
            return Err(format!("{name}: location query {guid}"));
        }
    }
    let objectives = entry["objectives"].as_array().ok_or("objectives")?;
    let ids: HashSet<&str> = objectives
        .iter()
        .filter_map(|objective| objective["id"].as_str())
        .collect();
    for objective in objectives {
        if let Some(parent) = objective["parentId"].as_str()
            && !ids.contains(parent)
        {
            return Err(format!("{name}: objective parent {parent}"));
        }
    }
    for modifier in entry["modifiers"].as_array().ok_or("modifiers")? {
        if let Some(guid) = modifier["brokerEntryGuid"].as_str()
            && !entries.contains(guid)
        {
            return Err(format!("{name}: modifier names entry {guid}"));
        }
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_mission_broker_entries_of_a_real_archive_resolve() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let unlisted = check_broker_links(&catalog)?;
    let entries = catalog["missionBrokerEntries"]
        .as_array()
        .ok_or("entries")?;
    let named: HashSet<&str> = catalog["contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|contract| contract["brokerEntryGuid"].as_str())
        .collect();
    let count = |field: &str| -> usize {
        entries
            .iter()
            .map(|entry| entry[field].as_array().map_or(0, Vec::len))
            .sum()
    };
    let outside = entries
        .iter()
        .filter(|entry| {
            !entry["recordPath"]
                .as_str()
                .is_some_and(|path| path.starts_with("libs/foundry/records/missionbroker/"))
        })
        .count();
    print_line(&format!(
        "{} mission broker entries ({outside} outside the missionbroker folder), {} named by a contract, {} objectives, {} properties, {} modifiers, {} flow triggers, {} requirement terms; links to Records the catalog does not list: {unlisted:?}",
        entries.len(),
        named.len(),
        count("objectives"),
        count("missionProperties"),
        count("modifiers"),
        count("missionFlow"),
        count("standingRequirements"),
    ));
    print_line(&format!(
        "the kind is {} bytes of JSON of the {} bytes of the catalog",
        serde_json::to_string(&catalog["missionBrokerEntries"])?.len(),
        built.json().len(),
    ));
    assert!(entries.len() > 2_000, "entries");
    assert!(!named.is_empty(), "a contract names an entry");
    assert!(count("objectives") > 500, "objectives");
    let failed: Vec<String> = built
        .summary()
        .failures()
        .iter()
        .map(|failure| format!("{}: {}", failure.record_path, failure.reason))
        .collect();
    assert!(
        failed.is_empty(),
        "records that cannot be read: {failed:#?}"
    );
    Ok(())
}
