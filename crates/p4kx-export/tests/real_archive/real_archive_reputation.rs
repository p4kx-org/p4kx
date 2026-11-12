// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Factions, reputation, mission types, localities, scenarios and the standing of contracts in the
//! Basetool catalog of a real Archive; runs with `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::catalog_v3_consumer;
use crate::real_archive_verification::guids as canonical_guids;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

fn catalog() -> Result<Value, Box<dyn Error>> {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    Ok(serde_json::from_slice(built.json())?)
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

fn named<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, Box<dyn Error>> {
    list(catalog, kind)?
        .iter()
        .find(|entry| {
            text(entry, "/className").is_some_and(|name| name.eq_ignore_ascii_case(class))
        })
        .ok_or_else(|| format!("{kind} has no {class}").into())
}

fn faction<'catalog>(
    catalog: &'catalog Value,
    class: &str,
) -> Result<&'catalog Value, Box<dyn Error>> {
    catalog_v3_consumer::rows(catalog, Kind::FactionReputations)?
        .iter()
        .find(|row| {
            text(row, "/className/value").is_some_and(|name| name.eq_ignore_ascii_case(class))
        })
        .ok_or_else(|| format!("faction reputation {class} is missing").into())
}

fn faction_lawful(owner: &Value) -> Option<bool> {
    let property = owner
        .pointer("/propertiesBb/value")?
        .as_array()?
        .iter()
        .find(|slot| text(slot, "/value/name/value") == Some("entityLawful"))?;
    let body = property.pointer("/value/dynamicProperty/value")?;
    if text(body, "/kind") != Some("boolean") {
        return None;
    }
    body.pointer("/value/value").and_then(Value::as_bool)
}

fn faction_name<'catalog>(catalog: &'catalog Value, owner: &Value) -> Option<&'catalog str> {
    let id = text(owner, "/displayName/value")?;
    let row = catalog
        .pointer("/texts/gameTexts")?
        .as_array()?
        .iter()
        .find(|row| text(row, "/id/value") == Some(id))?;
    text(row, "/translations/value/en/displayText/value")
}

fn references<'owner>(owner: &'owner Value, field: &str) -> impl Iterator<Item = &'owner str> {
    owner
        .get(field)
        .and_then(|carrier| carrier.get("value"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|slot| text(slot, "/value/guid/value"))
}

fn text<'c>(entry: &'c Value, pointer: &str) -> Option<&'c str> {
    entry.pointer(pointer).and_then(Value::as_str)
}

fn strings<'c>(entry: &'c Value, field: &str) -> Vec<&'c str> {
    entry[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
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

/// Asserts that the standings of every scope and the scopes of every context are kept Records.
fn check_ladders(catalog: &Value, scopes: &HashSet<&str>, standings: &HashSet<&str>) -> TestResult {
    for scope in catalog_v3_consumer::rows(catalog, Kind::ReputationScopes)? {
        assert_resolve(
            "the standings of a scope",
            scope
                .pointer("/standingMap/value/standings/value")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|link| text(link, "/value/guid/value")),
            standings,
        );
    }
    for context in catalog_v3_consumer::rows(catalog, Kind::ReputationContexts)? {
        assert_resolve(
            "the scopes of a context",
            context
                .pointer("/scopeContextList/value")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|slot| text(slot, "/value/scope/value/guid/value")),
            scopes,
        );
        assert_resolve(
            "the primary scope",
            text(context, "/primaryScopeContext/value/scope/value/guid/value"),
            scopes,
        );
    }
    Ok(())
}

fn check_faction_samples(catalog: &Value) -> TestResult {
    let wikelo = faction(catalog, "FactionReputation_Wikelo")?;
    assert_eq!(
        faction_lawful(wikelo),
        Some(true),
        "the entityLawful property"
    );
    assert!(
        faction_name(catalog, wikelo).is_some_and(|name| !name.is_empty()),
        "a name"
    );
    let headhunters = faction(catalog, "FactionReputation_Unlawful_Headhunters")?;
    assert_eq!(
        faction_lawful(headhunters),
        Some(false),
        "the original entityLawful value"
    );
    assert!(
        references(headhunters, "enemies").next().is_some(),
        "the Headhunters have enemies"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_factions_and_their_reputation_ladders_of_a_real_archive_are_linked() -> TestResult {
    let catalog = catalog()?;
    let factions = catalog_v3_consumer::rows(&catalog, Kind::FactionReputations)?;
    let scopes = canonical_guids(&catalog, "reputationScopes")?;
    let standings = canonical_guids(&catalog, "reputationStandings")?;
    let contexts = canonical_guids(&catalog, "reputationContexts")?;
    let faction_guids = canonical_guids(&catalog, "factionReputations")?;
    print_line(&format!(
        "{} factions, {} scopes, {} standings, {} contexts",
        factions.len(),
        scopes.len(),
        standings.len(),
        contexts.len()
    ));
    assert!(factions.len() >= 30, "the factions");
    assert!(scopes.len() >= 30, "the scopes");
    assert!(standings.len() >= 300, "the standings");

    check_faction_samples(&catalog)?;

    for faction in factions {
        assert_resolve("the allies", references(faction, "allies"), &faction_guids);
        assert_resolve(
            "the enemies",
            references(faction, "enemies"),
            &faction_guids,
        );
        assert_resolve(
            "the context",
            text(faction, "/reputationContextPropertiesUi/value/guid/value"),
            &contexts,
        );
        for relation in ["hostilityParams", "alliedParams"] {
            let scope = text(
                faction,
                &format!("/{relation}/value/scope/value/guid/value"),
            );
            let standing = text(
                faction,
                &format!("/{relation}/value/standing/value/guid/value"),
            );
            assert_resolve("the scope of a relation", scope, &scopes);
            assert_resolve("the standing of a relation", standing, &standings);
        }
    }
    check_ladders(&catalog, &scopes, &standings)?;
    let ranked = catalog_v3_consumer::rows(&catalog, Kind::ReputationStandings)?
        .iter()
        .filter(|standing| standing["minReputation"]["state"] == "present")
        .count();
    assert_eq!(ranked, standings.len(), "every standing has a minimum");
    Ok(())
}

/// Asserts that the faction, scope and standings a requirement names are kept Records.
fn check_requirement(requirement: &Value, [factions, scopes, standings]: [&HashSet<&str>; 3]) {
    assert_resolve(
        "the faction of a requirement",
        text(requirement, "/factionReputationGuid"),
        factions,
    );
    assert_resolve(
        "the scope of a requirement",
        text(requirement, "/scopeGuid"),
        scopes,
    );
    let range = [
        text(requirement, "/minStandingGuid"),
        text(requirement, "/maxStandingGuid"),
    ];
    assert_resolve(
        "the standings of a requirement",
        range.into_iter().flatten(),
        standings,
    );
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_contract_of_a_real_archive_names_its_type_and_standing_range() -> TestResult {
    let catalog = catalog()?;
    let contracts = crate::real_archive_contracts::v3::rows(&catalog)?;
    let types = guids(&catalog, "missionTypes")?;
    let standings = canonical_guids(&catalog, "reputationStandings")?;
    let scopes = canonical_guids(&catalog, "reputationScopes")?;
    let factions = canonical_guids(&catalog, "factionReputations")?;
    let ranged: Vec<&Value> = contracts
        .iter()
        .filter(|entry| entry["minStandingGuid"].is_string())
        .collect();
    print_line(&format!(
        "{} of {} contracts have a standing range; sample: {}",
        ranged.len(),
        contracts.len(),
        ranged.first().map_or(String::new(), ToString::to_string)
    ));
    assert!(ranged.len() > 1_000, "most contracts are gated by standing");
    let mut requirements = 0_usize;
    for entry in contracts {
        assert_resolve("the mission type", text(entry, "/missionTypeGuid"), &types);
        assert_resolve(
            "the minimum standing",
            text(entry, "/minStandingGuid"),
            &standings,
        );
        assert_resolve(
            "the maximum standing",
            text(entry, "/maxStandingGuid"),
            &standings,
        );
        for requirement in entry["standingRequirements"]
            .as_array()
            .into_iter()
            .flatten()
        {
            requirements += 1;
            check_requirement(requirement, [&factions, &scopes, &standings]);
        }
    }
    assert!(
        requirements > 500,
        "reputation prerequisites: {requirements}"
    );
    let wikelo_guid = text(
        faction(&catalog, "FactionReputation_Wikelo")?,
        "/guid/value",
    );
    let wikelo = contracts
        .iter()
        .find(|entry| {
            entry["standingRequirements"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|requirement| text(requirement, "/factionReputationGuid") == wikelo_guid)
        })
        .ok_or("a contract that requires standing with Wikelo")?;
    print_line(&format!("contract sample: {wikelo}"));
    Ok(())
}

/// Asserts that every faction of `scenario` links to a kept faction reputation.
fn check_scenario_factions(scenario: &Value, factions: &HashSet<&str>) {
    for faction in scenario["factionRewardTiers"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let linked = text(faction, "/factionReputationGuid");
        assert!(linked.is_some(), "the Faction Record names a reputation");
        assert_resolve("the faction of a scenario", linked, factions);
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn mission_types_localities_and_scenarios_of_a_real_archive_are_linked() -> TestResult {
    let catalog = catalog()?;
    let types = list(&catalog, "missionTypes")?;
    let localities = list(&catalog, "missionLocalities")?;
    let scenarios = list(&catalog, "contractScenarios")?;
    let objects = canonical_guids(&catalog, "starmapObjects")?;
    let pools = guids(&catalog, "blueprintPools")?;
    let factions = canonical_guids(&catalog, "factionReputations")?;
    print_line(&format!(
        "{} mission types, {} localities, {} scenarios",
        types.len(),
        localities.len(),
        scenarios.len()
    ));
    assert!(types.len() >= 30, "the mission types");
    assert!(
        types
            .iter()
            .filter(|kind| text(kind, "/typeName/en").is_some())
            .count()
            >= 20,
        "most mission types have an English name"
    );
    assert_eq!(localities.len(), 20, "the mission localities");
    let pyro = named(&catalog, "missionLocalities", "pyro")?;
    print_line(&format!("locality sample: {pyro}"));
    for locality in localities {
        let listed = strings(locality, "starmapObjectGuids");
        assert!(!listed.is_empty(), "a locality names objects");
        assert_resolve("the objects of a locality", listed, &objects);
    }
    assert_eq!(scenarios.len(), 2, "the scenarios");
    for scenario in scenarios {
        print_line(&format!("scenario sample: {scenario}"));
        let tiers: Vec<&Value> = scenario["factionRewardTiers"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|faction| faction["progressions"].as_array().into_iter().flatten())
            .flat_map(|progression| progression["tiers"].as_array().into_iter().flatten())
            .collect();
        assert!(!tiers.is_empty(), "a scenario has tiers");
        check_scenario_factions(scenario, &factions);
        for tier in tiers {
            assert_ne!(tier["badgeToAward"], "None", "no badge is null");
            assert!(tier["minPoints"].is_i64(), "points");
            assert_resolve(
                "the pools of a tier",
                strings(tier, "blueprintPoolGuids"),
                &pools,
            );
        }
    }
    Ok(())
}
