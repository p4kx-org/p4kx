// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The places, the giver faction and the blueprint rewards of the contracts in the Basetool
//! catalog of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_contracts::{
    reputation_gain_amount, reputation_gain_text, reputation_gains,
};
use crate::real_archive_verification::print_line;

type TestResult = Result<(), Box<dyn Error>>;

fn catalog() -> Result<Value, Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    Ok(serde_json::from_slice(built.json())?)
}

fn guids<'a>(value: &'a Value, field: &str) -> HashSet<&'a str> {
    value[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["guid"].as_str())
        .collect()
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn every_contract_links_its_places_and_reports_pickup_list_lengths() -> TestResult {
    let catalog = catalog()?;
    let localities = guids(&catalog, "missionLocalities");
    let objects = crate::real_archive_verification::guids(&catalog, "starmapObjects")?;
    let contracts = crate::real_archive_contracts::v3::rows(&catalog)?;
    let (mut with_places, mut max_pickup, mut unlinked) = (0_usize, 0_usize, 0_usize);
    for entry in contracts {
        let offered = &entry["offeredAt"];
        let linked = offered["localityGuids"].as_array().ok_or("localityGuids")?;
        let located = offered["locations"].as_array().ok_or("locations")?;
        for guid in linked {
            if !localities.contains(guid.as_str().unwrap_or_default()) {
                unlinked += 1;
            }
        }
        for place in located {
            let guid = place["starmapObjectGuid"].as_str().unwrap_or_default();
            if !objects.contains(guid) {
                unlinked += 1;
            }
        }
        if !linked.is_empty() || !located.is_empty() {
            with_places += 1;
        }
        let pickup = entry["pickupPlaces"]
            .as_array()
            .ok_or("pickupPlaces")?
            .len();
        if pickup > 0 {
            assert!(
                !linked.is_empty() || !located.is_empty(),
                "a contract with a pickup place states where it is offered"
            );
        }
        max_pickup = max_pickup.max(pickup);
    }
    print_line(&format!(
        "{} entries, {with_places} state a place, largest pickup list {max_pickup}, {unlinked} links without an entry",
        contracts.len()
    ));
    let added: usize = contracts
        .iter()
        .map(|entry| {
            ["offeredAt", "giverFaction", "blueprintRewards"]
                .iter()
                .map(|key| key.len() + entry[*key].to_string().len() + 4)
                .sum::<usize>()
        })
        .sum();
    print_line(&format!(
        "the three new fields add about {added} bytes of JSON"
    ));
    assert!(with_places > 0, "some contract is offered somewhere");
    assert_eq!(unlinked, 0, "every link resolves in this Archive");
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_giver_faction_follows_the_handler_then_a_positive_result_then_a_prerequisite() -> TestResult
{
    let catalog = catalog()?;
    let contracts = crate::real_archive_contracts::v3::rows(&catalog)?;
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for entry in contracts {
        let giver = &entry["giverFaction"];
        let Some(source) = giver["source"].as_str() else {
            assert_eq!(
                entry["faction"],
                Value::Null,
                "a handler faction is a giver"
            );
            continue;
        };
        *counts.entry(source).or_default() += 1;
        let guid = &giver["faction"]["guid"];
        match source {
            "handler" => assert_eq!(giver["faction"], entry["faction"]),
            "reputationResult" => {
                assert_eq!(entry["faction"], Value::Null);
                assert!(
                    has_positive_result_for(entry, guid.as_str().ok_or("giver faction GUID")?)?,
                    "a positive success amount of that faction"
                );
            }
            "reputationPrerequisite" => {
                assert_eq!(entry["faction"], Value::Null);
                assert!(
                    entry["standingRequirements"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|req| req["factionReputationGuid"] == *guid && req["exclude"] != true),
                    "a requirement of that faction"
                );
            }
            other => return Err(format!("source {other}").into()),
        }
    }
    print_line(&format!(
        "{} entries; giver sources {counts:?}",
        contracts.len()
    ));
    assert_eq!(counts.get("handler"), Some(&4010));
    assert_eq!(counts.get("reputationResult"), Some(&232));
    assert_eq!(counts.get("reputationPrerequisite"), Some(&57));
    print_line(&result_giver_census(contracts)?);
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_blueprint_rewards_of_an_entry_match_the_reward_sources_of_the_pools() -> TestResult {
    let catalog = catalog()?;
    let contracts = crate::real_archive_contracts::v3::rows(&catalog)?;
    let pools = catalog["blueprintPools"]
        .as_array()
        .ok_or("blueprintPools")?;
    let mut sources: HashMap<&str, HashSet<&str>> = HashMap::new();
    for pool in pools {
        for source in pool["rewardSources"].as_array().into_iter().flatten() {
            if let (Some(contract), Some(guid)) =
                (source["contractId"].as_str(), pool["guid"].as_str())
            {
                sources.entry(contract).or_default().insert(guid);
            }
        }
    }
    let mut entries = 0_usize;
    let mut listed: HashMap<&str, HashSet<&str>> = HashMap::new();
    for entry in contracts {
        let (contract, sub) = crate::real_archive_contracts::v3::identity(entry)?;
        let rewards = entry["blueprintRewards"]
            .as_array()
            .ok_or("blueprintRewards")?;
        let found = listed.entry(contract).or_default();
        for reward in rewards {
            let pool = reward["poolGuid"].as_str().unwrap_or_default();
            assert!(
                sources
                    .get(contract)
                    .is_some_and(|pools| pools.contains(pool)),
                "{contract} lists a pool that no reward source states"
            );
            found.insert(pool);
            entries += 1;
        }
        if sub.is_none() {
            continue;
        }
        let own = rewards
            .iter()
            .filter_map(|reward| reward["poolGuid"].as_str())
            .collect::<HashSet<_>>();
        assert_eq!(
            &own, found,
            "a sub contract of {contract} shares the results of its contract"
        );
    }
    let missed: usize = sources
        .iter()
        .filter_map(|(contract, pools)| Some(pools.difference(listed.get(contract)?).count()))
        .sum();
    print_line(&format!(
        "{entries} blueprint rewards on {} contracts; {missed} reward sources are not in an entry",
        listed.values().filter(|found| !found.is_empty()).count()
    ));
    assert_eq!(
        missed, 3,
        "exactly the three results that fire on the second outcome are left out"
    );
    assert!(entries > 0, "some entry hands out a blueprint");
    Ok(())
}

fn has_positive_result_for(entry: &Value, guid: &str) -> Result<bool, Box<dyn Error>> {
    for gain in reputation_gains(entry)? {
        if reputation_gain_text(gain, "factionReputationGuid")? == Some(guid)
            && reputation_gain_amount(gain)?.is_some_and(|amount| amount > 0)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Counts the `reputationResult` entries with more than one distinct positive faction and the
/// scopes of the picked gains.
#[expect(
    clippy::indexing_slicing,
    reason = "indexing a JSON value by key yields null, never a panic"
)]
fn result_giver_census(contracts: &[Value]) -> Result<String, Box<dyn Error>> {
    let mut several = 0_usize;
    let mut scopes: HashMap<String, usize> = HashMap::new();
    for entry in contracts {
        if entry["giverFaction"]["source"] != "reputationResult" {
            continue;
        }
        let giver = entry["giverFaction"]["faction"]["guid"]
            .as_str()
            .ok_or("giver faction GUID")?;
        let mut factions = HashSet::new();
        let mut picked_scope = None;
        for gain in reputation_gains(entry)? {
            if reputation_gain_amount(gain)?.is_some_and(|amount| amount > 0)
                && let Some(faction) = reputation_gain_text(gain, "factionReputationGuid")?
            {
                factions.insert(faction);
                if picked_scope.is_none() && faction == giver {
                    picked_scope = Some(reputation_gain_text(gain, "scopeName")?.unwrap_or("none"));
                }
            }
        }
        several += usize::from(factions.len() > 1);
        *scopes
            .entry(picked_scope.unwrap_or("none").to_owned())
            .or_default() += 1;
    }
    Ok(format!(
        "reputationResult: {several} entries with several positive factions; picked scopes {scopes:?}"
    ))
}
