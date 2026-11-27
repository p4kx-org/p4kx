// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::durability;
use super::record_targets::RecordTargets;
use super::suit_armor;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

#[path = "durability_suit_fixture.rs"]
mod fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reduced_health_retains_duplicate_physical_owners_and_exact_number_states() -> TestResult {
    with_queries(&fixture::complete(), |parsed, scratch| {
        let data = serde_json::to_value(durability::collect(parsed, scratch, None)?)?;
        let [first, repeated, alias] = rows(&data)? else {
            return Err("three selected Health components".into());
        };
        for (component, owner, slot) in [(first, 1, 1), (repeated, 1, 3), (alias, 3, 2)] {
            assert_eq!(
                component.pointer("/ownerRecordIndex/value"),
                Some(&json!(owner))
            );
            assert_eq!(
                component.pointer("/componentOrdinal/value"),
                Some(&json!(slot))
            );
            assert_eq!(
                component.pointer("/health/containsEncryptedSource"),
                Some(&json!(true))
            );
            assert_eq!(component.as_object().map(serde_json::Map::len), Some(4));
        }
        assert_eq!(
            first.get("ownerGuid"),
            alias.get("ownerGuid"),
            "physical aliases share the GUID"
        );
        assert_eq!(
            first.pointer("/health/value"),
            Some(&json!({"type":"f32", "finite":100.0}))
        );
        assert_eq!(
            repeated.pointer("/health/value"),
            Some(&json!({"type":"f32", "finite":null, "nonFinite":"naN"})),
            "a non-finite authored Health remains distinguishable"
        );
        let alias_value = alias.pointer("/health/value").ok_or("last exact Health")?;
        assert_eq!(
            serde_json::to_string(alias_value)?,
            r#"{"finite":-0.0,"type":"f32"}"#
        );
        Ok(())
    })
}

#[test]
fn reduced_suit_links_close_to_original_physicals_and_keep_explicit_absences() -> TestResult {
    with_queries(&fixture::complete(), |parsed, scratch| {
        let armor = suit_armor::collect(parsed, scratch, None)?;
        let suits = serde_json::to_value(armor.suits)?;
        let macros = serde_json::to_value(armor.macros)?;
        let [primary, alias] = rows(&macros)? else {
            return Err("two physical definitions".into());
        };
        let [linked, null, dangling, wrong_type] = rows(&suits)? else {
            return Err("four actual suit slots".into());
        };
        assert_eq!(
            primary.pointer("/source/guid/value"),
            alias.pointer("/source/guid/value")
        );
        assert_ne!(
            primary.pointer("/source/recordIndex/value"),
            alias.pointer("/source/recordIndex/value")
        );
        assert_eq!(
            linked.pointer("/damageResistance/value/targetRecordIndex"),
            primary.pointer("/source/recordIndex"),
            "same-GUID alias cannot satisfy the physical reference"
        );
        assert_eq!(
            linked.pointer("/damageResistance/value/status"),
            Some(&json!("present"))
        );
        assert_eq!(linked.pointer("/componentOrdinal/value"), Some(&json!(2)));
        assert_eq!(
            null.pointer("/damageResistance/state"),
            Some(&json!("null"))
        );
        assert_eq!(
            dangling.pointer("/damageResistance/value/status"),
            Some(&json!("dangling"))
        );
        assert_eq!(
            dangling.pointer("/damageResistance/value/targetRecordIndex/state"),
            Some(&json!("absent"))
        );
        assert_eq!(
            wrong_type.pointer("/damageResistance/value/status"),
            Some(&json!("unexpectedTargetType"))
        );
        assert!(linked.get("integrityMilestoneToBreak").is_none());
        verify_resistance(primary, alias)
    })
}

fn verify_resistance(primary: &Value, alias: &Value) -> TestResult {
    let body = primary
        .pointer("/damageResistance/value")
        .ok_or("selected macro body")?;
    for (name, multiplier) in [
        ("physical", 0.625),
        ("energy", 0.75),
        ("distortion", 0.25),
        ("biochemical", -0.5),
    ] {
        let path = format!("/{name}/value");
        let channel = body.pointer(&path).ok_or("selected resistance channel")?;
        for (field, expected) in [
            ("multiplier", multiplier),
            ("threshold", 2.0),
            ("damageCap", 80.0),
            ("damageCapLimit", 25.0),
        ] {
            let path = format!("/{field}");
            assert_eq!(
                channel.pointer(&path),
                Some(
                    &json!({"state":"present", "value":{"type":"f32","finite":expected}, "containsEncryptedSource":true})
                ),
                "{name} {field} has the original value and marker"
            );
        }
    }
    assert_eq!(
        body.pointer("/thermal/value/multiplier/value"),
        Some(&json!({"type":"f32", "finite":null, "nonFinite":"positiveInfinity"}))
    );
    assert_eq!(body.pointer("/stun/state"), Some(&json!("absent")));
    assert_eq!(
        alias.pointer("/damageResistance/value/energy/state"),
        Some(&json!("null"))
    );
    assert_eq!(
        alias.pointer("/damageResistance/value/physical/value/multiplier/value/finite"),
        Some(&json!(0.25))
    );
    for omitted in ["ignoreMeleeDamage", "additionalFields", "sourceType"] {
        assert!(body.get(omitted).is_none());
    }
    Ok(())
}

#[test]
fn selected_unclosed_armor_or_invalid_health_refuses_completion() -> TestResult {
    with_queries(&fixture::complete(), |parsed, scratch| {
        let error = suit_armor::collect_suits(parsed, scratch, None, &RecordTargets::default());
        assert!(
            matches!(
                error,
                Err(ReducedError::UnresolvedRecordLink {
                    path: suit_armor::PATH,
                    index: 0
                })
            ),
            "the linked physical macro must be in the output"
        );
        Ok(())
    })?;
    with_queries(&fixture::invalid_health(), |parsed, scratch| {
        assert!(
            matches!(
                durability::collect(parsed, scratch, None),
                Err(ReducedError::IncompleteScope(durability::PATH))
            ),
            "a selected invalid Health value cannot become authored absence"
        );
        Ok(())
    })
}

fn rows(table: &Value) -> Result<&[Value], &'static str> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or("staged equipment rows")
}

fn with_queries(
    database: &DataForgeBuilder,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    inspect(&session.parsed(None)?, scratch.path())
}
