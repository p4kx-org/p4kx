// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The links of the ship spawns and location queries of contracts in the Basetool catalog of a
//! real Archive.

use std::collections::HashSet;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_verification::{TestResult, guids, print_line};

/// The contracts whose location prerequisite names a variable no location query states: the four
/// PVP bounties of North Rock, whose `MissionLocation` is not a location property.
const KNOWN_DANGLING: &str = "NorthRock_Bounty";

/// Asserts that every ship spawn link and location query link of a contract names an entry of
/// its kind and that every location prerequisite names a variable of the contract's
/// `locationQueries`, apart from the known exceptions.
pub(crate) fn check_spawn_links(catalog: &Value) -> TestResult {
    let pools = guids(catalog, "contractShipSpawns")?;
    let queries = guids(catalog, "contractLocationQueries")?;
    for contract in catalog["contracts"].as_array().ok_or("contracts")? {
        for link in contract["shipSpawns"].as_array().ok_or("shipSpawns")? {
            let guid = link["poolGuid"].as_str().ok_or("poolGuid")?;
            assert!(pools.contains(guid), "ship spawn pool {guid}");
        }
        let mut variables = HashSet::new();
        for link in contract["locationQueries"]
            .as_array()
            .ok_or("locationQueries")?
        {
            let guid = link["queryGuid"].as_str().ok_or("queryGuid")?;
            assert!(queries.contains(guid), "location query {guid}");
            variables.insert(link["variable"].as_str().ok_or("variable")?);
        }
        for prerequisite in contract["locationPrerequisites"]
            .as_array()
            .ok_or("locationPrerequisites")?
        {
            let variable = prerequisite["variable"].as_str().ok_or("variable")?;
            let name = contract["debugName"].as_str().unwrap_or_default();
            assert!(
                variables.contains(variable) || name.starts_with(KNOWN_DANGLING),
                "{name}: location prerequisite {variable} names no location query"
            );
        }
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_spawn_links_of_a_real_archive_resolve() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    check_spawn_links(&catalog)?;
    let count = |kind: &str| catalog[kind].as_array().map_or(0, Vec::len);
    let links = |field: &str| -> usize {
        catalog["contracts"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|contract| contract[field].as_array().map_or(0, Vec::len))
            .sum()
    };
    print_line(&format!(
        "{} ship spawns, {} location queries, {} location templates; links: {} ship, {} query, {} prerequisites",
        count("contractShipSpawns"),
        count("contractLocationQueries"),
        count("missionLocationTemplates"),
        links("shipSpawns"),
        links("locationQueries"),
        links("locationPrerequisites"),
    ));
    assert!(count("contractShipSpawns") > 100, "ship spawns");
    assert!(count("contractLocationQueries") > 500, "location queries");
    assert!(
        count("missionLocationTemplates") > 1_000,
        "location templates"
    );
    assert!(
        links("shipSpawns") > 0 && links("locationQueries") > 0,
        "links"
    );
    Ok(())
}
