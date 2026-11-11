// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reputation and payout awarded for successful contracts.

use serde_json::{Value, json};

use super::{A, B, TestResult, v3};

#[test]
fn only_the_reputation_a_contract_grants_on_success_is_listed() -> TestResult {
    let (_, catalog) = super::build(None)?;
    let [faction, affinity] = result_rows(super::entry(&catalog, A, None)?, "reputationGains")?
    else {
        return Err("only the two successful reputation gains are listed".into());
    };
    for gain in [faction, affinity] {
        assert_eq!(gain.as_object().map(serde_json::Map::len), Some(5));
    }
    for (field, faction_expected, affinity_expected) in [
        (
            "factionReputationGuid",
            "60000000-0000-4000-8000-000000000001",
            "60000000-0000-4000-8000-000000000001",
        ),
        ("factionReputationName", "acme", "acme"),
        (
            "scopeGuid",
            "60000000-0000-4000-8000-000000000002",
            "60000000-0000-4000-8000-000000000003",
        ),
        ("scopeName", "faction", "affinity"),
        ("amount", "200", "50"),
    ] {
        assert_eq!(
            v3::cell(faction, field)?.and_then(Value::as_str),
            Some(faction_expected),
            "faction scope keeps {field}"
        );
        assert_eq!(
            v3::cell(affinity, field)?.and_then(Value::as_str),
            Some(affinity_expected),
            "affinity scope keeps {field}"
        );
    }
    assert!(
        result_rows(super::entry(&catalog, B, None)?, "reputationGains")?.is_empty(),
        "the contract grants no reputation"
    );
    Ok(())
}

#[test]
fn a_payout_the_contract_states_is_listed_as_stored_and_only_for_success() -> TestResult {
    let (_, catalog) = super::build(None)?;
    let [full, base] = result_rows(super::entry(&catalog, A, None)?, "fixedRewards")? else {
        return Err("only the two successful payouts are listed".into());
    };
    for (field, full_expected, base_expected) in [
        ("currency", Some(json!("UEC")), None),
        ("amount", Some(json!("40000")), Some(json!("7"))),
        ("max", Some(json!("0")), None),
        ("plusBonuses", Some(json!(false)), None),
    ] {
        assert_eq!(
            v3::cell(full, field)?,
            full_expected.as_ref(),
            "the first payout retains its original {field}"
        );
        assert_eq!(
            v3::cell(base, field)?,
            base_expected.as_ref(),
            "the second payout retains its original nullable {field}"
        );
    }
    assert!(
        result_rows(super::entry(&catalog, B, None)?, "fixedRewards")?.is_empty(),
        "the contract states no fixed payout"
    );
    Ok(())
}

fn result_rows<'row>(row: &'row Value, field: &str) -> TestResult<&'row [Value]> {
    v3::cell(row, field)?
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("expected an available {field} list").into())
}
