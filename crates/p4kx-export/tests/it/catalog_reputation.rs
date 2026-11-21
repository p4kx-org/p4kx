// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Factions, reputation scopes, standings and contexts, mission types and localities and contract
//! scenarios in the P4KX catalog.

#[path = "reputation_faction_owner_fixture.rs"]
mod owner_fixture;

#[path = "catalog_faction.rs"]
mod catalog_faction;
#[path = "catalog_faction_modifier.rs"]
mod faction_modifier;
#[path = "reputation_faction_owner.rs"]
mod faction_reputation_owner;
#[path = "catalog_scenario_faction_scope.rs"]
mod faction_scope;
#[path = "mission_type_texts_tests.rs"]
mod mission_type_texts;
#[path = "catalog_reputation_context.rs"]
mod reputation_context;
#[path = "reputation_perks.rs"]
mod reputation_perks;
#[path = "reputation_sandbox.rs"]
mod reputation_sandbox;
#[path = "catalog_reputation_scope.rs"]
mod reputation_scope;
#[path = "catalog_reputation_sdk.rs"]
mod sdk;

use crate::reputation_fixture::{
    CONTEXT, FACTION, LEGACY_FACTION, LOCALITY, MISSING, MISSION_TYPE, PLAIN_A, PLAIN_B, SCENARIO,
    SCOPE, STANDING_HIGH, STANDING_LOW, write_archive, write_archive_with_drift,
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
    let context = proof::at(&catalog, "/definitions/reputationContexts/rows/0")?;
    proof::present(context, "guid", &json!(guid(CONTEXT)), Some(true))?;
    assert_eq!(
        proof::at(context, "/primaryScopeContext/value/scope/value/guid/value")?,
        &json!(guid(SCOPE))
    );
    let slots = proof::rows(context, "/scopeContextList/value")?;
    let links = slots
        .iter()
        .map(|row| proof::at(row, "/value/scope/value/guid/value"))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        links,
        vec![&json!(guid(SCOPE)), &json!(guid(PLAIN_A))],
        "listed in stored order"
    );
    Ok(())
}

#[test]
fn a_mission_type_and_a_locality_keep_their_links() -> TestResult {
    let (_, catalog) = build(None)?;
    let id = json!(guid(MISSION_TYPE));
    let mission_type = proof::rows(&catalog, "/definitions/missionTypes/rows")?
        .iter()
        .find(|row| row.pointer("/guid/value") == Some(&id))
        .ok_or("mission type definition")?;
    assert_eq!(
        mission_type.pointer("/className/value"),
        Some(&json!("missiontype.job")),
        "source class name"
    );
    assert_eq!(
        mission_type.pointer("/recordPath/value"),
        Some(&json!("libs/foundry/records/missiontype/missiontype.job")),
        "normalized source record path"
    );
    let reference = proof::cell(mission_type, "typeName", "present")?;
    let row = mission_type_texts::protected(&catalog, reference)?;
    assert_eq!(row.pointer("/key/value"), Some(&json!("@type_job")));
    assert_eq!(
        row.pointer("/translations/value/en/displayText/value"),
        Some(&json!("Job")),
    );
    check_locality(&catalog)
}

fn check_locality(catalog: &Value) -> TestResult {
    let [row] = proof::rows(catalog, "/definitions/missionLocalities/rows")? else {
        return Err("exactly one physical locality".into());
    };
    let identity = proof::present(row, "guid", &json!(guid(LOCALITY)), Some(true))?;
    let index = serde_json::from_value::<u32>(proof::at(row, "/recordIndex/value")?.clone())?;
    proof::header(catalog, identity, index, "guid", "payload", "archiveEntry")?;
    let slots = proof::rows(row, "/availableLocations/value")?;
    let expected = [
        Some((PLAIN_A, "present")),
        Some((MISSING, "dangling")),
        None,
        Some((PLAIN_B, "present")),
        Some((PLAIN_A, "present")),
    ];
    assert_eq!(slots.len(), expected.len(), "every original slot survives");
    for (ordinal, (slot, expected)) in slots.iter().zip(expected).enumerate() {
        let slot_number = ordinal + 1;
        let field = if matches!(expected, Some((_, "dangling"))) {
            "$error.guid"
        } else {
            "$value"
        };
        let location = json!({
            "type": "subresource",
            "namespace": "dataforgeValue",
            "indices": [index.to_string(), slot_number.to_string()],
            "field": field
        });
        let inputs = proof::uses(catalog, slot)?;
        assert!(
            inputs
                .iter()
                .any(|used| used["location"] == location && used["role"] == "selector"),
            "each reference slot retains its exact original array coordinate"
        );
        if let Some((target, status)) = expected {
            let link = proof::at(slot, "/value")?;
            proof::present(link, "guid", &json!(guid(target)), Some(true))?;
            assert_eq!(
                link["status"], status,
                "the target resolution state survives"
            );
        } else {
            assert_eq!(slot["state"], "null", "the empty authored slot survives");
        }
    }
    proof::closure(catalog)
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
    assert_eq!(
        proof::rows(&catalog, "/definitions/missionTypes/rows")?.len(),
        1,
        "only the selected mission type"
    );
    assert_eq!(
        proof::at(&catalog, "/definitions/factionReputations/rows")?,
        &json!([])
    );
    assert_eq!(
        catalog["completeness"]["factionReputations"],
        json!({"status": "partial", "filter": "*/missiontype/*"})
    );
    assert_eq!(counts(&built, Kind::MissionTypes), Some((1, 0)));
    Ok(())
}
