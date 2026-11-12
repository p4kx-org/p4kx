// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Factions, reputation scopes, standings and contexts, mission types and localities and contract
//! scenarios in the Basetool catalog.

#[path = "catalog_faction.rs"]
mod catalog_faction;
#[path = "catalog_faction_modifier.rs"]
mod faction_modifier;
#[path = "catalog_scenario_faction_scope.rs"]
mod faction_scope;
#[path = "reputation_perks.rs"]
mod reputation_perks;
#[path = "catalog_reputation_scope.rs"]
mod reputation_scope;

use crate::reputation_fixture::{
    CONTEXT, FACTION, FACTION_BARE, LEGACY_FACTION, LOCALITY, MISSION_TYPE, PLAIN_A, PLAIN_B,
    SCENARIO, SCOPE, STANDING_HIGH, STANDING_LOW, write_archive, write_archive_with_drift,
};

use crate::health_origin_assertions as proof;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, Filter, Kind, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn build(filter: Option<&Filter>) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    build_with_archive(filter, write_archive)
}

fn build_with_archive(
    filter: Option<&Filter>,
    write: impl FnOnce(&Path) -> Result<PathBuf, Box<dyn Error>>,
) -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(write(dir.path())?, Some(p4kx_testkit::test_key()))?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.filter = filter;
    let built = build_catalog(&archive, &options)?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn guid(value: u128) -> String {
    let text = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    )
}

fn entry<'a>(catalog: &'a Value, kind: &str, record: u128) -> Result<&'a Value, Box<dyn Error>> {
    catalog[kind]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["guid"] == guid(record).as_str())
        .ok_or_else(|| format!("{kind} has no {}", guid(record)).into())
}

fn counts(built: &BuiltCatalog, kind: Kind) -> Option<(usize, usize)> {
    built
        .summary()
        .kind(kind)
        .map(|kind| (kind.count, kind.skipped))
}

#[test]
fn a_faction_lists_its_links_and_reads_lawful_from_the_property() -> TestResult {
    let (built, catalog) = build(None)?;
    assert_eq!(
        *entry(&catalog, "factionReputations", FACTION)?,
        json!({
            "guid": guid(FACTION),
            "className": "FactionReputation_Acme",
            "recordPath": "libs/foundry/records/factions/factionreputation/factionreputation_acme",
            "displayName": {"key": "@faction_acme", "en": "Acme Emporium", "de": null},
            "isNpc": false,
            "lawful": true,
            "contextGuid": guid(CONTEXT),
            "allyGuids": [guid(FACTION_BARE)],
            "enemyGuids": [],
            "hostility": {"scopeGuid": guid(SCOPE), "standingGuid": guid(STANDING_LOW)},
            "allied": {"scopeGuid": guid(SCOPE), "standingGuid": guid(STANDING_HIGH)}
        })
    );
    assert_eq!(
        counts(&built, Kind::FactionReputations),
        Some((2, 1)),
        "the test faction is a leftover"
    );
    Ok(())
}

#[test]
fn a_faction_without_the_fields_has_null_and_empty_values() -> TestResult {
    let (_, catalog) = build(None)?;
    let bare = entry(&catalog, "factionReputations", FACTION_BARE)?;
    for field in ["lawful", "contextGuid", "hostility", "allied"] {
        assert_eq!(bare[field], Value::Null, "{field}");
    }
    assert_eq!(bare["isNpc"], false, "the DataForge stores a default");
    assert_eq!(bare["allyGuids"], json!([]));
    assert_eq!(bare["enemyGuids"], json!([]));
    Ok(())
}

#[test]
fn a_scope_lists_its_standings_in_stored_order() -> TestResult {
    let (_, catalog) = build(None)?;
    let scope = proof::at(&catalog, "/definitions/reputationScopes/rows/0")?;
    proof::present(scope, "scopeName", &json!("FactionReputation"), Some(true))?;
    let rows = proof::rows(scope, "/standingMap/value/standings/value")?;
    let guids = rows
        .iter()
        .map(|row| proof::at(row, "/value/guid/value"))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        guids,
        vec![&json!(guid(STANDING_LOW)), &json!(guid(STANDING_HIGH))]
    );
    Ok(())
}

#[test]
fn a_standing_preserves_the_original_fields_and_protected_texts() -> TestResult {
    let (built, catalog) = build(None)?;
    let high = standing(&catalog, STANDING_HIGH)?;
    let low = standing(&catalog, STANDING_LOW)?;
    assert!(catalog.get("reputationStandings").is_none());
    proof::present(high, "guid", &json!(guid(STANDING_HIGH)), Some(true))?;
    proof::present(
        high,
        "className",
        &json!("ReputationStanding_Faction_B0_01"),
        Some(true),
    )?;
    proof::present(
        high,
        "minReputation",
        &json!({"type":"int64","value":"4500"}),
        Some(true),
    )?;
    proof::present(
        high,
        "driftReputation",
        &json!({"type":"int64","value":"-5"}),
        Some(true),
    )?;
    proof::present(
        high,
        "driftTimeHours",
        &json!({"rawBits":"41c40000","finite":24.5}),
        Some(true),
    )?;
    proof::present(
        low,
        "driftTimeHours",
        &json!({"rawBits":"80000000","finite":-0.0}),
        Some(true),
    )?;
    assert_eq!(low["driftTimeHours"]["value"]["finite"].to_string(), "-0.0");
    proof::present(high, "gated", &json!(true), Some(true))?;
    proof::present(low, "gated", &json!(false), Some(true))?;
    proof::present(
        high,
        "description",
        &json!("Synthetic standing description"),
        Some(true),
    )?;
    proof::present(high, "icon", &json!("ui/fixture/standing.dds"), Some(true))?;
    proof::present(
        high,
        "missionBoardIcon",
        &json!("ui/fixture/board.dds"),
        Some(true),
    )?;
    check_standing_texts(&catalog, high)?;
    assert_eq!(counts(&built, Kind::ReputationStandings), Some((2, 1)));
    proof::closure(&catalog)?;
    Ok(())
}

fn check_standing_texts(catalog: &Value, high: &Value) -> TestResult {
    for (field, key, expected) in [
        ("displayName", "@LOC_PLACEHOLDER", Value::Null),
        (
            "perkDescription",
            "@fixture_standing_perk",
            json!("Synthetic perk"),
        ),
    ] {
        let identifier = proof::cell(high, field, "present")?
            .get("value")
            .ok_or("protected standing text reference")?;
        let protected = proof::rows(catalog, "/texts/gameTexts")?
            .iter()
            .find(|row| row.pointer("/id/value") == Some(identifier))
            .ok_or("protected standing text is registered")?;
        assert_eq!(
            protected.pointer("/key/value").and_then(Value::as_str),
            Some(key),
            "original localization key for {field}"
        );
        assert_eq!(
            protected
                .pointer("/translations/value/en/displayText/value")
                .unwrap_or(&Value::Null),
            &expected,
            "English translation for {field}"
        );
    }
    Ok(())
}

#[test]
fn a_nonfinite_reputation_drift_preserves_its_exact_source_word() -> TestResult {
    let (_, catalog) = build_with_archive(None, |dir| {
        write_archive_with_drift(dir, f32::from_bits(0x7fc0_1073))
    })?;
    proof::present(
        standing(&catalog, STANDING_HIGH)?,
        "driftTimeHours",
        &json!({"rawBits":"7fc01073","finite":null}),
        Some(true),
    )?;
    assert_eq!(catalog["exportHealth"]["status"], "complete");
    proof::closure(&catalog)?;
    Ok(())
}

fn standing(catalog: &Value, record: u128) -> Result<&Value, Box<dyn Error>> {
    let identity = guid(record);
    proof::rows(catalog, "/definitions/reputationStandings/rows")?
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(&identity))
        .ok_or_else(|| format!("missing standing {identity}").into())
}

#[test]
fn a_context_names_its_primary_and_listed_scopes() -> TestResult {
    let (_, catalog) = build(None)?;
    let context = entry(&catalog, "reputationContexts", CONTEXT)?;
    assert_eq!(context["primaryScopeGuid"], guid(SCOPE).as_str());
    assert_eq!(
        context["scopeGuids"],
        json!([guid(SCOPE), guid(PLAIN_A)]),
        "listed in stored order"
    );
    Ok(())
}

#[test]
fn a_mission_type_and_a_locality_keep_their_links() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        *entry(&catalog, "missionTypes", MISSION_TYPE)?,
        json!({
            "guid": guid(MISSION_TYPE),
            "className": "missiontype.job",
            "recordPath": "libs/foundry/records/missiontype/missiontype.job",
            "typeName": {"key": "@type_job", "en": "Job", "de": null}
        })
    );
    assert_eq!(
        entry(&catalog, "missionLocalities", LOCALITY)?["starmapObjectGuids"],
        json!([guid(PLAIN_A), guid(PLAIN_B)]),
        "a Reference to no Record is left out"
    );
    Ok(())
}

#[test]
fn a_scenario_lists_the_tiers_of_each_faction() -> TestResult {
    let (_, catalog) = build(None)?;
    assert_eq!(
        *entry(&catalog, "contractScenarios", SCENARIO)?,
        json!({
            "guid": guid(SCENARIO),
            "className": "ORS_ScenarioProgress",
            "recordPath": "libs/foundry/records/contracts/contractscenarios/ors_scenarioprogress",
            "factionRewardTiers": [{
                "factionGuid": guid(LEGACY_FACTION),
                "factionReputationGuid": guid(FACTION),
                "factionName": "faction_lawful_crusaderindustries",
                "pointsAsPercent": false,
                "progressions": [{
                    "minMultiplierThreshold": 1.0,
                    "completionType": "CompletionType_GlobalProgression",
                    "periodScenarioGuid": null,
                    "tiers": [
                        {
                            "minPoints": 10_800,
                            "badgeToAward": "ORS_Pistol",
                            "globalPointsMultiplier": 1.0,
                            "blueprintPoolGuids": []
                        },
                        {
                            "minPoints": 18_000,
                            "badgeToAward": null,
                            "globalPointsMultiplier": 1.0,
                            "blueprintPoolGuids": [guid(PLAIN_B)]
                        }
                    ]
                }]
            }]
        })
    );
    Ok(())
}

#[test]
fn a_filter_marks_the_kinds_partial_and_selects_by_record_path() -> TestResult {
    let filter = Filter::new("*/missiontype/*");
    let (built, catalog) = build(Some(&filter))?;
    assert_eq!(catalog["missionTypes"].as_array().map(Vec::len), Some(1));
    assert_eq!(catalog["factionReputations"], json!([]));
    assert_eq!(
        catalog["completeness"]["factionReputations"],
        json!({"status": "partial", "filter": "*/missiontype/*"})
    );
    assert_eq!(counts(&built, Kind::MissionTypes), Some((1, 0)));
    Ok(())
}
