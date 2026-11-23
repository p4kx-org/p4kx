// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Mission types, localities and scenarios in the installed catalog.

use std::collections::HashSet;

use p4kx_export::Kind;
use serde_json::Value;

use super::{TestResult, assert_resolve, catalog, list, print_line, text};
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};
use crate::real_archive_verification::guids as canonical_guids;

fn slots<'row>(row: &'row Value, path: &str) -> impl Iterator<Item = &'row Value> {
    row.pointer(path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|slot| slot.get("value").filter(|value| !value.is_null()))
}

/// Asserts that resolved faction reputations join the matching Catalog.
fn check_scenario_factions(scenario: &Value, factions: &HashSet<&str>) {
    for faction in slots(scenario, "/factionRewardTiers/value") {
        let linked = text(faction, "/factionReputation/value/guid/value");
        if text(faction, "/factionReputation/value/status") == Some("present") {
            assert!(
                linked.is_some(),
                "a resolved Faction reputation retains its GUID"
            );
            assert_resolve("the faction of a scenario", linked, factions);
        }
    }
}

fn check_scenario_tiers(scenario: &Value, pools: &HashSet<&str>) {
    let tiers: Vec<_> = slots(scenario, "/factionRewardTiers/value")
        .flat_map(|faction| slots(faction, "/progressions/value"))
        .flat_map(|progression| slots(progression, "/tiers/value"))
        .collect();
    assert!(
        !tiers.is_empty(),
        "the scenario retains authored reward tiers"
    );
    for tier in tiers {
        assert!(
            tier.pointer("/minPoints/value/type")
                .and_then(Value::as_str)
                .is_some(),
            "authored points retain their actual numeric representation"
        );
        let pool_guids = slots(tier, "/blueprintPools/value")
            .filter(|link| text(link, "/status") == Some("present"))
            .filter_map(|link| text(link, "/guid/value"));
        assert_resolve("the resolved pools of a tier", pool_guids, pools);
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
    let types = crate::catalog_v3_consumer::rows(&catalog, Kind::MissionTypes)?;
    let localities = crate::catalog_v3_consumer::rows(&catalog, Kind::MissionLocalities)?;
    let scenarios = list(&catalog, "contractScenarios")?;
    let objects = canonical_guids(&catalog, "starmapObjects")?;
    let pools = canonical_guids(&catalog, "blueprintPools")?;
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
    let pyro = localities
        .iter()
        .find(|row| {
            text(row, "/className/value").is_some_and(|name| name.eq_ignore_ascii_case("pyro"))
        })
        .ok_or("the canonical locality for Pyro")?;
    print_line(&format!("locality sample: {pyro}"));
    for locality in localities {
        let slots = locality
            .pointer("/availableLocations/value")
            .and_then(Value::as_array)
            .ok_or("the original location slots")?;
        let listed = slots
            .iter()
            .filter(|slot| text(slot, "/value/status") == Some("present"))
            .map(|slot| text(slot, "/value/guid/value").ok_or("resolved location GUID"))
            .collect::<Result<Vec<_>, _>>()?;
        assert!(!listed.is_empty(), "a locality names objects");
        assert_resolve("the objects of a locality", listed, &objects);
    }
    assert!(
        !scenarios.is_empty(),
        "the installed Build supplies scenario progressions"
    );
    for scenario in scenarios {
        check_scenario_factions(scenario, &factions);
        check_scenario_tiers(scenario, &pools);
    }
    Ok(())
}
