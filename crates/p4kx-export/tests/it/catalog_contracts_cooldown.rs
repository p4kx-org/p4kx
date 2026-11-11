// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The cooldown fields of contracts in the Basetool catalog.

use crate::catalog_contracts::{A, B, SUB_ONE, TestResult, build, entry, v3};

use serde_json::{Value, json};

#[test]
fn a_contract_and_its_sub_contracts_carry_the_cooldown_of_their_handler() -> TestResult {
    let (_, catalog) = build(None)?;
    let stated = json!({
        "cooldownLayer": "handler",
        "hasPersonalCooldown": true,
        "personalCooldownTime": 2880.0,
        "personalCooldownTimeVariation": 60.0,
        "abandonedCooldownTime": 30.0,
        "abandonedCooldownTimeVariation": 0.5,
    });
    for (guid, sub) in [(A, None), (A, Some(SUB_ONE)), (B, None)] {
        let contract = entry(&catalog, guid, sub)?;
        for (field, value) in stated.as_object().into_iter().flatten() {
            assert_eq!(
                v3::cell(contract, field)?,
                Some(value),
                "{field} of {guid} {sub:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_handler_that_states_some_fields_gives_null_for_the_rest_and_for_a_wrong_type() -> TestResult {
    let (_, catalog) = build(None)?;
    let event = v3::rows(&catalog)?
        .iter()
        .find(|entry| entry["handler"] == "Copy_Career")
        .ok_or("Copy_Career")?;
    assert_eq!(
        v3::cell(event, "cooldownLayer")?,
        Some(&json!("handler")),
        "the partial handler states a cooldown"
    );
    assert_eq!(
        v3::cell(event, "hasPersonalCooldown")?,
        Some(&Value::Bool(false)),
        "the explicit false survives"
    );
    for field in [
        "personalCooldownTime",
        "personalCooldownTimeVariation",
        "abandonedCooldownTime",
        "abandonedCooldownTimeVariation",
    ] {
        assert_eq!(
            v3::cell(event, field)?,
            None,
            "{field}: absent or not a Single"
        );
    }
    Ok(())
}

#[test]
fn a_handler_without_cooldown_fields_gives_null_and_no_layer() -> TestResult {
    let (_, catalog) = build(None)?;
    let bounty = v3::rows(&catalog)?
        .iter()
        .find(|entry| entry["handler"] == "Bounty_Career")
        .ok_or("Bounty_Career")?;
    for field in [
        "cooldownLayer",
        "hasPersonalCooldown",
        "personalCooldownTime",
        "personalCooldownTimeVariation",
        "abandonedCooldownTime",
        "abandonedCooldownTimeVariation",
    ] {
        assert_eq!(v3::cell(bounty, field)?, None, "{field}");
    }
    Ok(())
}
