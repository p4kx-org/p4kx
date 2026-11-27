// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::{
    AFFINITY, CONTRACT_A, CONTRACT_B, FACTION, FACTION_SCOPE, SUB_ONE, SUB_TWO, database, english,
    german,
};

#[path = "reputation_gain_source.rs"]
mod source;

use self::source::{Gain, Link, Original, Site};

#[test]
fn reputation_gains_keep_result_selection_and_the_actual_reward_origins() -> TestResult {
    let bytes = source::with_duplicate_reward(database()).build();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&bytes, input, vec![english(), german()])?;
        let original = source::read(&fixture.entry.open()?)?;
        let (_, catalog) = fixture.diagnostic()?;
        check_parent_and_inheriting_child(&catalog, &original, input)?;
        check_empty(&catalog, &original, input)?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn check_parent_and_inheriting_child(
    catalog: &Value,
    original: &Original,
    input: Input,
) -> TestResult {
    let parent = health_origin_fixture::guid(CONTRACT_A);
    let child = health_origin_fixture::guid(SUB_ONE);
    for sub in [None, Some(child.as_str())] {
        let row = catalog_contracts::entry(catalog, &parent, sub)?;
        let list = collection(row, input)?;
        let [faction, affinity] = origin::rows(list, "/value")? else {
            return Err("the two original successful reputation entries in order".into());
        };
        let membership = origin::uses(catalog, list)?;
        for selector in [original.success, original.failed_outcome] {
            assert_site(&membership, selector, "selector");
        }
        let [first, second] = &original.gains;
        check_gain(
            catalog,
            faction,
            first,
            (FACTION_SCOPE, "faction"),
            "9007199254740993",
            input,
        )?;
        check_gain(
            catalog,
            affinity,
            second,
            (AFFINITY, "affinity"),
            "50",
            input,
        )?;
        check_duplicate_selection(catalog, faction, original)?;
        assert!(
            membership
                .iter()
                .all(|used| !original.rejected_amount.matches(used)),
            "the failure payout does not contribute its external amount"
        );
    }
    Ok(())
}

fn check_gain(
    catalog: &Value,
    row: &Value,
    gain: &Gain,
    scope: (u128, &str),
    amount: &str,
    input: Input,
) -> TestResult {
    check_link(
        catalog,
        row,
        &gain.faction,
        ("factionReputationGuid", "factionReputationName"),
        (FACTION, "acme"),
        input,
    )?;
    check_link(
        catalog,
        row,
        &gain.scope,
        ("scopeGuid", "scopeName"),
        scope,
        input,
    )?;
    let cell = origin::present(row, "amount", &json!(amount), input.marker())?;
    let uses = origin::uses(catalog, cell)?;
    assert_site(&uses, gain.reward.caller, "selector");
    assert_site(&uses, gain.amount, "payload");
    Ok(())
}

fn check_link(
    catalog: &Value,
    row: &Value,
    link: &Link,
    fields: (&str, &str),
    expected: (u128, &str),
    input: Input,
) -> TestResult {
    let guid = origin::present(
        row,
        fields.0,
        &json!(health_origin_fixture::guid(expected.0)),
        input.marker(),
    )?;
    assert_site(&origin::uses(catalog, guid)?, link.caller, "payload");
    let name = origin::present(row, fields.1, &json!(expected.1), input.marker())?;
    let uses = origin::uses(catalog, name)?;
    assert_site(&uses, link.caller, "selector");
    assert_use(
        &uses,
        "dataforgeRecordHeader",
        link.target,
        "fileName",
        "conversion",
    );
    Ok(())
}

fn check_duplicate_selection(catalog: &Value, row: &Value, original: &Original) -> TestResult {
    let amount = origin::cell(row, "amount", "present")?;
    let uses = origin::uses(catalog, amount)?;
    assert!(
        uses.iter()
            .all(|used| !original.first_reward.matches(used)
                && !original.rejected_amount.matches(used)),
        "the raw first GUID target and unsuccessful result supply no selected amount"
    );
    assert_use(
        &uses,
        "dataforgeRecordDeclarations",
        original.later_empty,
        "$fields",
        "selector",
    );
    Ok(())
}

fn check_empty(catalog: &Value, original: &Original, input: Input) -> TestResult {
    let parent = health_origin_fixture::guid(CONTRACT_A);
    let child = health_origin_fixture::guid(SUB_TWO);
    let bare = health_origin_fixture::guid(CONTRACT_B);
    let rows = [
        (
            catalog_contracts::entry(catalog, &parent, Some(child.as_str()))?,
            original.child_results,
        ),
        (
            catalog_contracts::entry(catalog, &bare, None)?,
            original.bare_results,
        ),
    ];
    for (row, source) in rows {
        let list = origin::present(row, "reputationGains", &json!([]), input.marker())?;
        let uses = origin::uses(catalog, list)?;
        assert_site(&uses, source, "selector");
        assert!(
            uses.iter().all(|used| !original.success.matches(used)),
            "a child with its own nonempty result block does not borrow the parent's success"
        );
    }
    Ok(())
}

fn collection(row: &Value, input: Input) -> TestResult<&Value> {
    let list = origin::cell(row, "reputationGains", "present")?;
    assert_eq!(
        list.get("sourceSetComplete"),
        Some(&json!(true)),
        "the successful membership decisions are fully observed"
    );
    assert_eq!(
        list.get("containsEncryptedSource"),
        Some(&json!(input.marker())),
        "the collection uses its actual original Generator"
    );
    Ok(list)
}

fn assert_site(uses: &[&Value], site: Site, role: &str) {
    assert!(
        uses.iter().any(|used| {
            site.matches(used)
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
                && used.get("role").and_then(Value::as_str) == Some(role)
        }),
        "the physical value {site:?} contributes with role {role}"
    );
}

fn assert_use(uses: &[&Value], namespace: &str, record: u32, field: &str, role: &str) {
    let indices = json!([record.to_string()]);
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && used.pointer("/location/indices") == Some(&indices)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.get("role").and_then(Value::as_str) == Some(role)
        }),
        "the actual {namespace} {record}/{field} supplies {role}"
    );
}

#[path = "reputation_gain_candidate_sources.rs"]
mod candidate_sources;
