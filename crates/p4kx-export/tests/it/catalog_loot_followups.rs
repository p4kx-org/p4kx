// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The older loot Records and the warnings of the loot container scan, in the P4KX catalog.

#[path = "loot_generation_component_tests.rs"]
mod loot_generation_component_tests;

use crate::catalog_loot::{build, entry};
use crate::health_origin_assertions as proof;
use crate::loot_fixture::{
    GHOST, LEGACY_ARCHETYPE, LEGACY_TABLE, NESTED, TABLE, TAG_COMMON, TAG_RARE,
};
use crate::starmap_fixture::guid_text;

use std::error::Error;

use p4kx_export::{ExportDiagnosticCode, ExportDiagnosticSeverity, Kind};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn an_older_loot_table_keeps_its_weights_and_result_limits() -> TestResult {
    let (built, catalog) = build()?;
    let table = entry(&catalog, "legacyLootTables", LEGACY_TABLE)?;
    legacy_assert_fields(
        table,
        [
            ("/className/value", json!("Cargo")),
            (
                "/recordPath/value",
                json!("libs/foundry/records/lootgeneration/loottables/cargo"),
            ),
            (
                "/entries/value/0/value/weight/value",
                proof::single_number(2.0)?,
            ),
            (
                "/entries/value/0/value/archetypeGuid/value",
                json!(guid_text(LEGACY_ARCHETYPE)),
            ),
            ("/entries/value/0/value/minResults/value/value", json!(0)),
            ("/entries/value/0/value/maxResults/value/value", json!(3)),
            (
                "/entries/value/1/value/weight/value",
                proof::single_number(1.0)?,
            ),
            ("/entries/value/1/value/archetypeGuid/state", json!("null")),
            ("/entries/value/1/value/minResults/value/value", json!(0)),
            ("/entries/value/1/value/maxResults/value/value", json!(0)),
        ],
    )?;
    assert_eq!(
        proof::rows(table, "/entries/value")?.len(),
        2,
        "both weighted slots survive"
    );
    assert_eq!(
        built
            .summary()
            .kind(Kind::LegacyLootTables)
            .map(|kind| (kind.count, kind.skipped)),
        Some((1, 0))
    );
    Ok(())
}

#[test]
fn an_older_loot_archetype_keeps_its_tags_optional_data_and_groups() -> TestResult {
    let (_, catalog) = build()?;
    let archetype = entry(&catalog, "legacyLootArchetypes", LEGACY_ARCHETYPE)?;
    legacy_assert_fields(
        archetype,
        [
            ("/className/value", json!("LootArchetype_Cargo")),
            (
                "/excludedTagGuids/value/0/value",
                json!(guid_text(TAG_RARE)),
            ),
            (
                "/secondaryGroups/value/0/value/groupName/value",
                json!("Rarity"),
            ),
            (
                "/secondaryGroups/value/0/value/entries/value/0/value/weight/value",
                proof::single_number(4.0)?,
            ),
            (
                "/secondaryGroups/value/0/value/entries/value/0/value/tagGuid/value",
                json!(guid_text(TAG_RARE)),
            ),
        ],
    )?;
    let primary = proof::at(archetype, "/entries/value/0/value")?;
    legacy_assert_fields(
        primary,
        [
            ("/name/value", json!("Cargo")),
            ("/weight/value", proof::single_number(1.0)?),
            ("/tagGuid/value", json!(guid_text(TAG_COMMON))),
            (
                "/additionalPositiveTagGuids/value/0/value",
                json!(guid_text(TAG_RARE)),
            ),
            ("/additionalNegativeTagGuids/value", json!([])),
            ("/stackSize/value/min/value/value", json!(3)),
            ("/stackSize/value/max/value/value", json!(5)),
        ],
    )?;
    assert_legacy_spawn(proof::at(primary, "/spawnWith/value/0/value")?)?;
    Ok(())
}

fn assert_legacy_spawn(spawn: &Value) -> TestResult {
    legacy_assert_fields(
        spawn,
        [
            ("/mode/value", json!("MostSimilar")),
            ("/name/value", json!("Suitable ammunition")),
            ("/min/value/value", json!(1)),
            ("/max/value/value", json!(3)),
            (
                "/positiveTagGuids/value/0/value",
                json!(guid_text(TAG_COMMON)),
            ),
            (
                "/negativeTagGuids/value/0/value",
                json!(guid_text(TAG_RARE)),
            ),
        ],
    )
}

fn legacy_assert_fields(
    row: &Value,
    expected: impl IntoIterator<Item = (&'static str, Value)>,
) -> TestResult {
    for (path, expected) in expected {
        assert_eq!(
            proof::at(row, path)?,
            &expected,
            "the legacy authored fact at {path} survives"
        );
    }
    Ok(())
}

#[test]
fn an_older_record_is_no_v3_record_and_the_other_way_round() -> TestResult {
    let (_, catalog) = build()?;
    assert!(entry(&catalog, "lootTables", LEGACY_TABLE).is_err());
    assert!(entry(&catalog, "legacyLootTables", TABLE).is_err());
    Ok(())
}

fn warning_details(
    built: &p4kx_export::BuiltCatalog,
    kind: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    built
        .summary()
        .warnings()
        .iter()
        .find(|warning| warning.kind() == kind)
        .map(|warning| warning.details().to_vec())
        .ok_or_else(|| format!("no {kind} warning").into())
}

#[test]
fn a_placed_container_the_archive_lacks_is_warned_about() -> TestResult {
    let (built, catalog) = build()?;
    let health = built.summary().export_health();
    let warning = health
        .diagnostics()
        .iter()
        .find(|row| {
            row.kind == Some(Kind::LootContainers)
                && row.code == ExportDiagnosticCode::VerifiedMissingReference
        })
        .ok_or("verified missing container warning")?;
    assert_eq!(warning.severity, ExportDiagnosticSeverity::Warning);
    assert_eq!(warning.count, 1);
    assert!(
        health
            .failed_entries()
            .iter()
            .all(|entry| entry.kind() != Kind::LootContainers),
        "the proved absent target contributes no failed Entry operation"
    );
    let ghost = catalog["lootContainers"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|row| row.pointer("/container/value") == Some(&json!(GHOST)))
        .ok_or("the missing container has a row")?;
    assert_eq!(
        ghost.pointer("/entityCount/state"),
        Some(&json!("absent")),
        "the full lookup proves the count is absent"
    );
    Ok(())
}

#[test]
fn an_entity_file_with_several_containers_is_warned_about() -> TestResult {
    let (built, _) = build()?;
    let details = warning_details(&built, "ambiguous_loot_entity_files")?;
    assert_eq!(
        details,
        [format!("{NESTED}: entdata\\1.entxml")],
        "only the entity file with two different containers"
    );
    Ok(())
}
