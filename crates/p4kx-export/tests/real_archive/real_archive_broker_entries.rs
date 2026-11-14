// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The mission broker entries in the Basetool catalog of a real Archive.

use std::collections::{BTreeMap, HashSet};
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogSummary, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_contract_templates::mission_graph::{self as graph, Family, Mission};
use crate::real_archive_verification::{TestResult, guids, print_line};

/// Checks the successful contextual Broker owners, their original links and objective parents.
pub(crate) fn check_broker_links(catalog: &Value) -> Result<BTreeMap<&'static str, usize>, String> {
    let contexts = graph::contexts(catalog, Family::Broker)?;
    let entries: HashSet<_> = contexts
        .iter()
        .map(|mission| graph::guid(mission.definition))
        .collect::<Result<_, _>>()?;
    let contracts =
        crate::real_archive_contracts::v3::rows(catalog).map_err(|error| error.to_string())?;
    for contract in contracts {
        if let Some(guid) = contract["brokerEntryGuid"].as_str()
            && !entries.contains(guid)
        {
            return Err(format!("broker entry {guid} of a contract"));
        }
    }
    let pools = guids(catalog, "contractShipSpawns")?;
    let queries = guids(catalog, "contractLocationQueries")?;
    let types = guids(catalog, "missionTypes")?;
    let locations = guids(catalog, "starmapObjects")?;
    let localities = guids(catalog, "missionLocalities")?;
    let targets = [
        ("type", "missionTypeGuid", &types),
        ("locationMissionAvailable", "locationGuid", &locations),
        ("localityAvailable", "localityGuid", &localities),
        ("invitationMission", "invitationMissionGuid", &entries),
        ("linkedMission", "linkedMissionGuid", &entries),
    ];
    let mut unlisted = BTreeMap::new();
    for mission in &contexts {
        for (field, label, listed) in &targets {
            if let Some(value) = mission.root(field)?
                && let Some(guid) = graph::reference(value)?
                && !listed.contains(guid)
            {
                *unlisted.entry(*label).or_insert(0) += 1;
            }
        }
        mission_lists(mission, &entries, &mut unlisted)?;
        check_parts(mission, (&pools, &queries, &entries))?;
    }
    Ok(unlisted)
}

fn mission_lists(
    mission: &Mission<'_>,
    entries: &HashSet<&str>,
    unlisted: &mut BTreeMap<&'static str, usize>,
) -> Result<(), String> {
    for field in [
        "associatedMissions",
        "requiredMissions",
        "onlyAvailableIfAllMissionsNotAvailable",
    ] {
        if let Some(array) = mission.root(field)? {
            for value in mission.elements(array)? {
                if let Some(guid) = graph::reference(value)?
                    && !entries.contains(guid)
                {
                    *unlisted.entry("missionGuids").or_insert(0) += 1;
                }
            }
        }
    }
    Ok(())
}

fn check_parts(
    mission: &Mission<'_>,
    (pools, queries, entries): (&HashSet<&str>, &HashSet<&str>, &HashSet<&str>),
) -> Result<(), String> {
    let name = graph::text(mission.definition, "/className/value")?;
    for link in mission.pools()? {
        match link.family {
            "ships" if !pools.contains(link.guid) => {
                return Err(format!("{name}: ship spawn pool {}", link.guid));
            }
            "ships" if link.level != "broker" => {
                return Err(format!("{name}: level of a ship spawn"));
            }
            "location" if !queries.contains(link.guid) => {
                return Err(format!("{name}: location query {}", link.guid));
            }
            _ => {}
        }
    }
    mission.parents()?;
    for row in graph::array(mission.projection, "modifiers")? {
        let modifier = mission.follow(row.get("source").ok_or("actual selected modifier")?)?;
        if let Some(value) = mission.member(modifier, "missionBrokerEntry")?
            && let Some(guid) = graph::reference(value)?
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
    assert!(
        unlisted.is_empty(),
        "links to Records the catalog does not list, per field: {unlisted:?}"
    );
    assert_nothing_unknown(built.summary());
    let entries = graph::contexts(&catalog, Family::Broker)?;
    let named: HashSet<&str> = crate::real_archive_contracts::v3::rows(&catalog)?
        .iter()
        .filter_map(|contract| contract["brokerEntryGuid"].as_str())
        .collect();
    let counts = entry_counts(&entries)?;
    let triggers = broker_triggers(&catalog)?;
    print_line(&format!(
        "{} mission broker entries ({} outside the missionbroker folder), {} named by a contract, {} objectives, {} properties, {} modifiers, {triggers} flow triggers, {} requirement terms; links to Records the catalog does not list: {unlisted:?}",
        entries.len(),
        counts.outside,
        named.len(),
        counts.objectives,
        counts.properties,
        counts.modifiers,
        counts.terms,
    ));
    let definition_table = catalog
        .pointer("/definitions/missionBrokerEntries")
        .ok_or("Broker D table")?;
    let context_table = catalog
        .pointer("/configurations/missionBrokerEntries")
        .ok_or("Broker C table")?;
    let size = serde_json::to_string(definition_table)?.len()
        + serde_json::to_string(context_table)?.len();
    print_line(&format!(
        "the Broker D/C tables are {size} bytes of JSON of the {} bytes of the catalog",
        built.json().len()
    ));
    assert!(entries.len() > 2_000, "entries");
    assert!(!named.is_empty(), "a contract names an entry");
    assert!(counts.objectives > 500, "objectives");
    Ok(())
}

#[derive(Default)]
struct Counts {
    outside: usize,
    objectives: usize,
    properties: usize,
    modifiers: usize,
    terms: usize,
}

fn entry_counts(entries: &[Mission<'_>]) -> Result<Counts, String> {
    let mut counts = Counts::default();
    for mission in entries {
        let row = mission.projection;
        counts.outside += usize::from(
            !graph::text(mission.definition, "/recordPath/value")?
                .starts_with("libs/foundry/records/missionbroker/"),
        );
        counts.objectives += graph::array(row, "objectives")?.len();
        counts.properties += graph::array(row, "properties")?
            .iter()
            .filter(|property| property.get("objective") == Some(&Value::Null))
            .count();
        counts.modifiers += graph::array(row, "modifiers")?.len();
        counts.terms += graph::array(row, "standingRequirements")?.len();
    }
    Ok(counts)
}

fn broker_triggers(catalog: &Value) -> Result<usize, String> {
    let rows = catalog
        .pointer("/configurations/missionFlows/rows")
        .and_then(Value::as_array)
        .ok_or("actual shared Flow table")?;
    rows.iter()
        .filter(|row| {
            row.pointer("/owner/type").and_then(Value::as_str) == Some("missionBrokerEntry")
        })
        .try_fold(0, |count, row| {
            Ok(count + graph::array(row, "triggers")?.len())
        })
}

/// Asserts that no Record failed and that no stored name of a template or a broker entry is
/// unknown to the catalog (the warning `unknown_template_values`).
fn assert_nothing_unknown(summary: &CatalogSummary) {
    let failed: Vec<String> = summary
        .failures()
        .iter()
        .map(|failure| format!("{}: {}", failure.record_path(), failure.reason()))
        .collect();
    assert!(
        failed.is_empty(),
        "records that cannot be read: {failed:#?}"
    );
    assert!(
        summary
            .warnings()
            .iter()
            .all(|warning| warning.kind() != "unknown_template_values"),
        "a stored name of a template or broker entry is unknown: {:?}",
        summary.warnings()
    );
}
