// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::{ReducedError, attachable_modifiers, shield_generators};
use crate::build::original_input::test_support::TestResult;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

#[path = "shield_salvage_fixture.rs"]
mod fixture;

#[test]
fn shield_generators_keep_physical_slots_and_ordered_resistance_numbers() -> TestResult {
    with_queries(fixture::Resistance::Present, |parsed, scratch| {
        let table = shield_generators::collect(parsed, scratch, None)?;
        let output = serde_json::to_value(table)?;
        let [first, second] = rows(&output)? else {
            return Err("two physical shield components on one Record".into());
        };
        for (row, ordinal, health) in [(first, 1, "9007199254740993"), (second, 3, "128")] {
            verify_owner(row, ordinal);
            verify_value(row, "/maxHealth", &json!({"type":"uint64","value":health}))?;
            verify_value(row, "/maxRegen", &json!({"type":"f32","finite":12.5}))?;
            verify_value(
                row,
                "/downedRegenDelay",
                &json!({"type":"f32","finite":4.0}),
            )?;
            verify_value(
                row,
                "/electricalChargeDamageResistance",
                &json!({"type":"f32","finite":0.25}),
            )?;
            verify_negative_zero(row, "/damagedRegenDelay")?;
            verify_resistances(row)?;
            for field in [
                "decayRatio",
                "shieldAbsorption",
                "stunParams",
                "additionalFields",
            ] {
                assert!(row.get(field).is_none(), "excluded shield field {field}");
            }
        }
        Ok(())
    })
}

#[test]
fn shield_resistance_keeps_known_absence_and_refuses_wrong_storage() -> TestResult {
    with_queries(fixture::Resistance::Absent, |parsed, scratch| {
        let output = serde_json::to_value(shield_generators::collect(parsed, scratch, None)?)?;
        for row in rows(&output)? {
            assert_eq!(
                row.pointer("/shieldResistance/state"),
                Some(&json!("absent")),
                "unavailable declaration"
            );
            assert_eq!(
                row.pointer("/shieldResistance/value"),
                Some(&Value::Null),
                "no invented range"
            );
        }
        Ok(())
    })?;
    with_queries(fixture::Resistance::Invalid, |parsed, scratch| {
        assert!(
            matches!(
                shield_generators::collect(parsed, scratch, None),
                Err(ReducedError::IncompleteScope(shield_generators::PATH))
            ),
            "a selected resistance declaration with wrong storage blocks the reduced table"
        );
        Ok(())
    })
}

#[test]
fn attachable_modifiers_keep_salvage_cells_at_original_effect_slots() -> TestResult {
    with_queries(fixture::Resistance::Present, |parsed, scratch| {
        let table = attachable_modifiers::collect(parsed, scratch, None)?;
        let output = serde_json::to_value(table)?;
        let [row] = rows(&output)? else {
            return Err("one original attachable modifier component".into());
        };
        verify_owner(row, 2);
        assert!(
            row.get("charges").is_none(),
            "activation data is outside the selected scope"
        );
        let [other, null, weapon] = row
            .pointer("/modifiers/value")
            .and_then(Value::as_array)
            .ok_or("authored effect slots")?
            .as_slice()
        else {
            return Err("original nonweapon, null and weapon slots".into());
        };
        assert_eq!(
            other.get("value"),
            Some(&json!({"type":"authored"})),
            "nonweapon effect preserves its slot and type"
        );
        assert_eq!(
            null.get("state"),
            Some(&json!("null")),
            "original null effect remains explicit"
        );
        assert_eq!(
            weapon.pointer("/value/type"),
            Some(&json!("weapon")),
            "exact typed weapon branch"
        );

        let salvage = weapon
            .pointer("/value/value/weaponModifier/value/weaponStats/value/salvageModifier/value")
            .ok_or("selected canonical salvage branch")?;
        verify_value(
            salvage,
            "/radiusMultiplier",
            &json!({"type":"f32","finite":3.5}),
        )?;
        verify_negative_zero(salvage, "/salvageSpeedMultiplier")?;
        verify_value(
            salvage,
            "/extractionEfficiency",
            &json!({"type":"f32","finite":null,"nonFinite":"naN"}),
        )?;
        assert!(
            weapon
                .pointer("/value/value/weaponModifier/value/weaponStats/value/damageMultiplier")
                .is_none(),
            "excluded weapon stats stay outside the selected path"
        );
        Ok(())
    })
}

fn verify_resistances(row: &Value) -> TestResult {
    let ranges = row
        .pointer("/shieldResistance/value")
        .and_then(Value::as_array)
        .ok_or("authored resistance ranges")?;
    let [first, repeated, third] = ranges.as_slice() else {
        return Err("all independent physical resistance positions".into());
    };
    assert_eq!(
        first, repeated,
        "equal authored ranges remain separate array entries"
    );
    verify_value(first, "/value/min", &json!({"type":"f32","finite":0.125}))?;
    verify_value(first, "/value/max", &json!({"type":"f32","finite":0.75}))?;
    verify_value(
        third,
        "/value/max",
        &json!({"type":"f32","finite":null,"nonFinite":"positiveInfinity"}),
    )
}

fn verify_owner(row: &Value, ordinal: u32) {
    assert_eq!(
        row.pointer("/ownerRecordIndex/value"),
        Some(&json!(0)),
        "physical owner index"
    );
    assert_eq!(
        row.pointer("/componentOrdinal/value"),
        Some(&json!(ordinal)),
        "original component position"
    );
    assert_eq!(
        row.pointer("/componentOrdinal/containsEncryptedSource"),
        Some(&json!(true)),
        "component position comes from the admitted encrypted DataForge"
    );
}

fn verify_value(row: &Value, field: &str, expected: &Value) -> TestResult {
    let cell = row.pointer(field).ok_or("selected numeric cell")?;
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "present source value at {field}"
    );
    assert_eq!(
        cell.get("value"),
        Some(expected),
        "exact numeric projection at {field}"
    );
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(true)),
        "original encryption marker at {field}"
    );
    assert!(
        cell.get("sourceUsesRefs").is_none(),
        "marker-only cell at {field}"
    );
    Ok(())
}

fn verify_negative_zero(row: &Value, field: &str) -> TestResult {
    let value = row
        .pointer(&format!("{field}/value"))
        .ok_or("selected signed zero value")?;
    assert_eq!(
        serde_json::to_string(value.get("finite").ok_or("finite value")?)?,
        "-0.0",
        "signed zero at {field}"
    );
    verify_value(row, field, &json!({"type":"f32","finite":-0.0}))
}

fn rows(table: &Value) -> TestResult<&[Value]> {
    table
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "staged equipment rows".into())
}

fn with_queries(
    resistance: fixture::Resistance,
    inspect: impl FnOnce(&ParsedCatalogQueries<'_>, &Path) -> TestResult,
) -> TestResult {
    let scratch = tempfile::tempdir()?;
    let input = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(
            TestEntry::stored("Data/Game2.dcb", fixture::database(resistance).build()).encrypted(),
        )
        .write_to(&input)?;
    let archive = Archive::open_with_key(input, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    inspect(&session.parsed(None)?, scratch.path())
}
