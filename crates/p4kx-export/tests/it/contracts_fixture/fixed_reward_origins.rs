// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::{CONTRACT_A, CONTRACT_B, SUB_TWO, database, english, german};

#[path = "fixed_reward_source.rs"]
mod source;

use self::source::{MALFORMED, Malformed, Ordinary, Site};

#[test]
fn fixed_rewards_keep_parent_order_and_actual_success_and_payload_sources() -> TestResult {
    let data = source::with_malformed(database()).build();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&data, input, vec![english(), german()])?;
        let forge = fixture.entry.open()?;
        let ordinary = source::ordinary(&forge)?;
        let malformed = source::malformed(&forge)?;
        let (_, catalog) = fixture.diagnostic()?;
        check_ordinary(&catalog, &ordinary, input)?;
        check_malformed(&catalog, &malformed, input)?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn check_ordinary(catalog: &Value, original: &Ordinary, input: Input) -> TestResult {
    let parent = health_origin_fixture::guid(CONTRACT_A);
    let child = health_origin_fixture::guid(SUB_TWO);
    for sub in [None, Some(child.as_str())] {
        let row = catalog_contracts::entry(catalog, &parent, sub)?;
        let list = collection(row, true, input.marker())?;
        check_parent_payouts(catalog, list, original, input)?;
    }
    let empty = catalog_contracts::entry(catalog, &health_origin_fixture::guid(CONTRACT_B), None)?;
    let list = collection(empty, true, input.marker())?;
    assert!(
        origin::rows(list, "/value")?.is_empty(),
        "the original empty result list is complete"
    );
    assert_use(
        &origin::uses(catalog, list)?,
        original.empty_results,
        "$value",
        "selector",
    );
    Ok(())
}

fn check_parent_payouts(
    catalog: &Value,
    list: &Value,
    original: &Ordinary,
    input: Input,
) -> TestResult {
    let [full, base] = origin::rows(list, "/value")? else {
        return Err("two parent payouts in original order".into());
    };
    let membership = origin::uses(catalog, list)?;
    for outcome in [
        original.full_success,
        original.base_success,
        original.failure,
    ] {
        assert_use(&membership, outcome, "$value", "selector");
    }
    assert!(
        membership
            .iter()
            .all(|used| !original.failed_amount.matches(used)
                && !original.child_result.matches(used)),
        "the list skips the failure payout and does not read the child's own results"
    );
    for (field, expected, payload) in [
        ("currency", json!("UEC"), original.currency),
        ("amount", json!("40000"), original.full_amount),
        ("max", json!("0"), original.maximum),
        ("plusBonuses", json!(false), original.bonuses),
    ] {
        assert_cell(
            catalog,
            full,
            field,
            &expected,
            payload,
            original.full_success,
            input,
        )?;
    }
    assert_cell(
        catalog,
        base,
        "amount",
        &json!("7"),
        original.base_amount,
        original.base_success,
        input,
    )?;
    for field in ["currency", "max", "plusBonuses"] {
        let missing = origin::cell(base, field, "absent")?;
        assert_meta(missing, true, input.marker());
        let uses = origin::uses(catalog, missing)?;
        assert_use(&uses, original.base_owner, "$fields", "selector");
        assert_use(&uses, original.base_success, "$value", "selector");
    }
    Ok(())
}

fn check_malformed(catalog: &Value, original: &Malformed, input: Input) -> TestResult {
    let row = catalog_contracts::entry(catalog, &health_origin_fixture::guid(MALFORMED), None)?;
    let marker = matches!(input, Input::Encrypted).then_some(true);
    let list = collection(row, false, marker)?;
    let [reward] = origin::rows(list, "/value")? else {
        return Err("the later true flag does not select the failed first flag's payout".into());
    };
    let membership = origin::uses(catalog, list)?;
    assert_use(&membership, original.success, "$value", "selector");
    assert!(
        membership
            .iter()
            .all(|used| !original.ignored_later_success.matches(used)
                && !original.ignored_amount.matches(used)),
        "a later outcome slot and an unselected payout cannot contribute"
    );
    assert_cell(
        catalog,
        reward,
        "amount",
        &json!("-9"),
        original.amount,
        original.success,
        input,
    )?;
    assert_cell(
        catalog,
        reward,
        "max",
        &json!("9007199254740993"),
        original.wide_maximum,
        original.success,
        input,
    )?;
    let unreadable = origin::cell(reward, "plusBonuses", "unreadable")?;
    assert_meta(unreadable, false, marker);
    let uses = origin::uses(catalog, unreadable)?;
    assert_use(&uses, original.success, "$value", "selector");
    assert_use(&uses, original.bonuses_error, "$fieldName", "selector");
    assert!(
        uses.iter().all(|used| {
            !original.bonuses_error.matches(used)
                || used.pointer("/location/field").and_then(Value::as_str) != Some("$value")
        }),
        "the unreadable value is excluded from the retained contributors"
    );
    let inventory: Vec<_> = origin::rows(catalog, "/provenance/uses")?.iter().collect();
    assert_use(&inventory, original.success_error, "$value", "attempted");
    assert_use(&inventory, original.bonuses_error, "$value", "attempted");
    Ok(())
}

fn collection(row: &Value, complete: bool, marker: Option<bool>) -> TestResult<&Value> {
    let value = origin::cell(row, "fixedRewards", "present")?;
    assert_meta(value, complete, marker);
    Ok(value)
}

fn assert_meta(value: &Value, complete: bool, marker: Option<bool>) {
    assert_eq!(
        value.get("sourceSetComplete"),
        Some(&json!(complete)),
        "membership and optional scalar sources each declare their own completeness"
    );
    assert_eq!(
        value.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "the actual contributing source determines the immediate marker"
    );
}

fn assert_cell(
    catalog: &Value,
    row: &Value,
    field: &str,
    expected: &Value,
    payload: Site,
    success: Site,
    input: Input,
) -> TestResult {
    let cell = origin::present(row, field, expected, input.marker())?;
    let uses = origin::uses(catalog, cell)?;
    assert_use(&uses, payload, "$value", "payload");
    assert_use(&uses, success, "$value", "selector");
    Ok(())
}

fn assert_use(uses: &[&Value], site: Site, field: &str, role: &str) {
    assert!(
        uses.iter().any(|used| {
            site.matches(used)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.get("role").and_then(Value::as_str) == Some(role)
        }),
        "the original {site:?} {field} supplies {role}"
    );
}
