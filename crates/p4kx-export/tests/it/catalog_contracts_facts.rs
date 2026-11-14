// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The contract facts in the Basetool catalog: time to complete, buy-in, sharing, legality,
//! availability and the required intro contracts.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::catalog_contracts::{TestResult, entry, v3};
use crate::facts_fixture;

pub(crate) const BARE: &str = "21000000-0000-4000-8000-000000000001";
pub(crate) const OVERRIDDEN: &str = "21000000-0000-4000-8000-000000000002";
const INTRO: &str = "21000000-0000-4000-8000-000000000003";
const GATED: &str = "21000000-0000-4000-8000-000000000004";
const STRIPPED: &str = "21000000-0000-4000-8000-000000000006";
pub(crate) const SUB: &str = "22000000-0000-4000-8000-000000000001";
pub(crate) const SUB_DIRECT: &str = "22000000-0000-4000-8000-000000000002";
const TAG: &str = "23000000-0000-4000-8000-000000000001";
const FAIL_TAG: &str = "23000000-0000-4000-8000-000000000002";

pub(crate) fn catalog() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        facts_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn at(value: impl Into<Value>, layer: &str) -> Value {
    json!({"value": value.into(), "layer": layer})
}

fn assert_facts(entry: &Value, expected: &Value) -> TestResult {
    let fields = expected.as_object().ok_or("expected contract facts")?;
    for (name, expected_fact) in fields {
        let fact = entry.get(name).ok_or(name.as_str())?;
        for field in ["value", "layer"] {
            assert_eq!(
                v3::cell(fact, field)?,
                expected_fact.get(field),
                "{name}.{field}"
            );
            if expected_fact.is_null() {
                let cell = fact.get(field).ok_or(field)?;
                assert_eq!(
                    cell.get("state").and_then(Value::as_str),
                    Some("absent"),
                    "{name}.{field} is readably absent"
                );
                assert_eq!(
                    cell.get("sourceSetComplete").and_then(Value::as_bool),
                    Some(true),
                    "{name}.{field} absence is fully observed"
                );
            }
        }
    }
    Ok(())
}

fn assert_run_terms(row: &Value, expected: Option<(f64, i64)>) -> TestResult {
    let time = v3::cell(row, "timeToComplete")?;
    let buy_in = v3::cell(row, "buyInAmount")?;
    if let Some((time_expected, buy_in_expected)) = expected {
        assert_eq!(
            time,
            Some(&json!(time_expected)),
            "the original completion time"
        );
        assert_eq!(
            buy_in,
            Some(&json!(buy_in_expected)),
            "the original buy-in amount"
        );
    } else {
        assert!(time.is_none(), "no time is stated");
        assert!(buy_in.is_none(), "no buy-in is stated");
    }
    Ok(())
}

#[test]
fn a_contract_without_overrides_takes_the_handler_and_the_template() -> TestResult {
    let catalog = catalog()?;
    let row = entry(&catalog, BARE, None)?;
    assert_run_terms(row, Some((0.0, 0)))?;
    assert_facts(
        row,
        &json!({
            "canBeShared": at(true, "template"),
            "illegal": at(false, "template"),
            "onceOnly": at(true, "handler"),
            "maxPlayersPerInstance": at(1, "handler"),
            "availableInPrison": at(false, "handler"),
            "canReacceptAfterAbandoning": at(true, "handler"),
            "canReacceptAfterFailing": at(false, "handler"),
            "hideInMobiGlas": at(true, "handler"),
        }),
    )
}

#[test]
fn a_contract_override_wins_over_the_handler_and_the_template() -> TestResult {
    let catalog = catalog()?;
    let row = entry(&catalog, OVERRIDDEN, None)?;
    assert_run_terms(row, Some((25.0, 1500)))?;
    assert_facts(
        row,
        &json!({
            "canBeShared": at(true, "template"),
            "illegal": at(true, "contract"),
            "onceOnly": at(false, "contract"),
            "maxPlayersPerInstance": at(4, "contract"),
        }),
    )
}

#[test]
fn a_sub_contract_override_wins_and_the_rest_follows_the_contract() -> TestResult {
    let catalog = catalog()?;
    let row = entry(&catalog, OVERRIDDEN, Some(SUB))?;
    assert_run_terms(row, Some((25.0, 1500)))?;
    assert_facts(
        row,
        &json!({
            "canBeShared": at(false, "subContract"),
            "illegal": at(true, "contract"),
            "onceOnly": at(false, "contract"),
        }),
    )
}

#[test]
fn a_sub_contract_states_its_parameters_directly() -> TestResult {
    let catalog = catalog()?;
    assert_facts(
        entry(&catalog, OVERRIDDEN, Some(SUB_DIRECT))?,
        &json!({"onceOnly": at(true, "subContract")}),
    )
}

fn assert_intros(row: &Value, expected: &Value) -> TestResult {
    let rows = v3::cell(row, "requiredIntros")?
        .and_then(Value::as_array)
        .ok_or("present required intro list")?;
    let actual = rows
        .iter()
        .map(intro_values)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        &json!(actual),
        expected,
        "authored prerequisite order and selected grants"
    );
    Ok(())
}

fn intro_values(row: &Value) -> Result<Value, Box<dyn Error>> {
    let level = v3::cell(row, "level")?.ok_or("present intro requirement layer")?;
    let tag = v3::cell(row, "tagGuid")?.ok_or("present intro requirement tag")?;
    let cells = v3::cell(row, "introContracts")?
        .and_then(Value::as_array)
        .ok_or("present grant list")?;
    let mut ids = Vec::new();
    for cell in cells {
        ids.push(v3::sourced(cell)?.ok_or("present intro grant id")?);
    }
    Ok(json!({"level": level, "tagGuid": tag, "introContracts": ids}))
}

#[test]
fn a_required_tag_lists_the_intro_contracts_that_grant_it_on_success() -> TestResult {
    let catalog = catalog()?;
    assert_intros(
        entry(&catalog, GATED, None)?,
        &json!([
            {"level": "handler", "tagGuid": FAIL_TAG, "introContracts": []},
            {"level": "contract", "tagGuid": TAG, "introContracts": [INTRO]},
            {"level": "contract", "tagGuid": FAIL_TAG, "introContracts": []},
        ]),
    )
}

#[test]
fn a_contract_that_requires_nothing_inherits_the_handler_requirement_only() -> TestResult {
    let catalog = catalog()?;
    assert_intros(
        entry(&catalog, BARE, None)?,
        &json!([{"level": "handler", "tagGuid": FAIL_TAG, "introContracts": []}]),
    )
}

#[test]
fn a_sub_contract_lists_the_tags_it_requires_after_those_of_its_handler_and_contract() -> TestResult
{
    let catalog = catalog()?;
    assert_intros(
        entry(&catalog, OVERRIDDEN, Some(SUB))?,
        &json!([
            {"level": "handler", "tagGuid": FAIL_TAG, "introContracts": []},
            {"level": "subContract", "tagGuid": TAG, "introContracts": [INTRO]},
        ]),
    )
}

#[test]
fn a_handler_parameter_wins_over_its_default_availability() -> TestResult {
    let catalog = catalog()?;
    assert_facts(
        entry(&catalog, BARE, None)?,
        &json!({"hideInMobiGlas": at(true, "handler")}),
    )
}

#[test]
fn a_contract_without_template_results_or_availability_has_absent_facts() -> TestResult {
    let catalog = catalog()?;
    let row = entry(&catalog, STRIPPED, None)?;
    assert_run_terms(row, None)?;
    assert_eq!(
        row.get("templateGuid"),
        Some(&Value::Null),
        "no template is declared"
    );
    assert_intros(row, &json!([]))?;
    assert_facts(
        row,
        &json!({
            "canBeShared": null,
            "illegal": null,
            "onceOnly": null,
            "maxPlayersPerInstance": null,
            "availableInPrison": null,
            "canReacceptAfterAbandoning": null,
            "canReacceptAfterFailing": null,
            "hideInMobiGlas": null,
        }),
    )
}
