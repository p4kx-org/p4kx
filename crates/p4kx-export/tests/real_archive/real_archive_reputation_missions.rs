// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mission types, localities and scenarios in the installed catalog.

use std::collections::HashSet;
use std::error::Error;

use serde_json::Value;

use super::{TestResult, assert_resolve, catalog, guids, list, print_line, text};
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};
use crate::real_archive_verification::guids as canonical_guids;

fn named<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, Box<dyn Error>> {
    list(catalog, kind)?
        .iter()
        .find(|entry| {
            text(entry, "/className").is_some_and(|name| name.eq_ignore_ascii_case(class))
        })
        .ok_or_else(|| format!("{kind} has no {class}").into())
}

fn strings<'c>(entry: &'c Value, field: &str) -> Vec<&'c str> {
    entry[field]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
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

fn check_mission_type_names(catalog: &Value, types: &[Value]) -> TestResult {
    let texts = TextIndex::new(catalog)?;
    let mut resolved = 0;
    for row in types {
        let field = row.get("typeName").ok_or("mission type name carrier")?;
        if let Some(target) =
            texts.reference(field, TextDomain::GameText, TextReferenceShape::Required)?
            && target
                .shown("en")?
                .as_str()
                .is_some_and(|name| !name.is_empty())
        {
            resolved += 1;
        }
    }
    assert!(resolved >= 20, "most mission types have an English name");
    Ok(())
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
    check_mission_type_names(&catalog, types)?;
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
