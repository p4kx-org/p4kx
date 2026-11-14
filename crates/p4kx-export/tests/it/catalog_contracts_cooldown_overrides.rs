// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The cooldown of contracts in the Basetool catalog when parameter overrides state it.

use serde_json::{Value, json};

use crate::catalog_contracts::{TestResult, entry, v3};
use crate::catalog_contracts_facts::{BARE, OVERRIDDEN, SUB, SUB_DIRECT, catalog};

#[test]
fn a_handler_default_is_stated_by_the_handler() -> TestResult {
    let catalog = catalog()?;
    assert_cooldown_fields(
        entry(&catalog, BARE, None)?,
        &json!({
            "cooldownLayer": "handler",
            "hasPersonalCooldown": true,
            "personalCooldownTime": 120.0,
            "personalCooldownTimeVariation": null,
            "cooldownLayers": {
                "hasPersonalCooldown": "handler",
                "personalCooldownTime": "handler",
                "personalCooldownTimeVariation": null,
                "abandonedCooldownTime": "handler",
                "abandonedCooldownTimeVariation": null,
            },
        }),
    )
}

#[test]
fn a_handler_parameter_wins_over_the_default_availability_of_the_cooldown() -> TestResult {
    let catalog = catalog()?;
    assert_eq!(
        v3::cell(entry(&catalog, BARE, None)?, "abandonedCooldownTime")?,
        Some(&json!(15.0)),
        "the handler parameter supplies the selected value"
    );
    Ok(())
}

#[test]
fn a_contract_override_wins_and_names_the_contract_as_the_layer() -> TestResult {
    let catalog = catalog()?;
    assert_cooldown_fields(
        entry(&catalog, OVERRIDDEN, None)?,
        &json!({
            "cooldownLayer": "contract",
            "hasPersonalCooldown": false,
            "personalCooldownTime": 45.0,
            "abandonedCooldownTime": 15.0,
            "cooldownLayers": {
                "hasPersonalCooldown": "contract",
                "personalCooldownTime": "contract",
                "personalCooldownTimeVariation": null,
                "abandonedCooldownTime": "handler",
                "abandonedCooldownTimeVariation": null,
            },
        }),
    )
}

#[test]
fn a_sub_contract_override_matches_the_name_without_regard_to_case() -> TestResult {
    let catalog = catalog()?;
    assert_cooldown_fields(
        entry(&catalog, OVERRIDDEN, Some(SUB))?,
        &json!({
            "cooldownLayer": "subContract",
            "hasPersonalCooldown": false,
            "personalCooldownTime": 7.0,
            "abandonedCooldownTime": 15.0,
            "cooldownLayers": {
                "hasPersonalCooldown": "contract",
                "personalCooldownTime": "subContract",
                "personalCooldownTimeVariation": null,
                "abandonedCooldownTime": "handler",
                "abandonedCooldownTimeVariation": null,
            },
        }),
    )
}

#[test]
fn a_sub_contract_without_overrides_follows_its_contract() -> TestResult {
    let catalog = catalog()?;
    assert_cooldown_fields(
        entry(&catalog, OVERRIDDEN, Some(SUB_DIRECT))?,
        &json!({"cooldownLayer": "contract", "personalCooldownTime": 45.0}),
    )
}

fn assert_cooldown_fields(row: &Value, expected: &Value) -> TestResult {
    let fields = expected.as_object().ok_or("expected cooldown fields")?;
    for (field, value) in fields {
        if field == "cooldownLayers" {
            let layers = row.get(field).ok_or("actual cooldown layers")?;
            assert_cooldown_layers(layers, value)?;
        } else {
            assert_eq!(
                v3::cell(row, field)?,
                (!value.is_null()).then_some(value),
                "{field}"
            );
        }
    }
    Ok(())
}

fn assert_cooldown_layers(layers: &Value, expected: &Value) -> TestResult {
    let actual = layers.as_object().ok_or("actual cooldown layer object")?;
    let expected = expected
        .as_object()
        .ok_or("expected cooldown layer object")?;
    assert_eq!(
        actual.len(),
        expected.len(),
        "the complete per-field layer map"
    );
    for (name, value) in expected {
        assert_eq!(
            v3::cell(layers, name)?,
            (!value.is_null()).then_some(value),
            "level of {name}"
        );
    }
    Ok(())
}
