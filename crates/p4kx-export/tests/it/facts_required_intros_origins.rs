// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestRecord, TestValue};
use serde_json::{Value, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

#[path = "facts_required_intro_source.rs"]
mod source;

use self::source::{Grant, Requirement, Site, Sources};
use super::{
    FAIL_TAG, GATED, GENERATOR, INTRO, OVERRIDDEN, SUB, TAG, contract, database, requires, terms,
};

const LOWER_INTRO: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0001;
const FAILED_INTRO: u128 = 0x2100_0000_0000_4000_8000_0000_0000_0010;
const REPEATED: u128 = 0x2100_0000_0000_4000_8000_0000_0000_0011;
const MISSING_TAG: u128 = 0x2300_0000_0000_4000_8000_0000_0000_0010;

#[test]
fn required_intro_fields_keep_original_requirements_and_successful_grant_sources() -> TestResult {
    let bytes = database().record(second_generator()).build();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&bytes, input, Vec::new())?;
        let original = Sources::read(&fixture)?;
        let (_, catalog) = fixture.diagnostic()?;
        check_inherited(&catalog, &original, input.marker())?;
        check_repeated(&catalog, &original, input.marker())?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn check_inherited(catalog: &Value, original: &Sources, marker: Option<bool>) -> TestResult {
    let row = catalog_contracts::entry(catalog, &health_origin_fixture::guid(GATED), None)?;
    let [handler, contract, ungranted] = requirements(row)? else {
        return Err("handler requirement followed by the two authored contract tags".into());
    };
    check_requirement(
        catalog,
        handler,
        original.handler,
        "handler",
        FAIL_TAG,
        marker,
    )?;
    check_requirement(
        catalog,
        contract,
        original.contract,
        "contract",
        TAG,
        marker,
    )?;
    check_requirement(
        catalog,
        ungranted,
        original.ungranted,
        "contract",
        FAIL_TAG,
        marker,
    )?;
    check_grants(catalog, contract, original, marker)?;
    check_no_grant(catalog, ungranted, original, marker)?;

    let parent = health_origin_fixture::guid(OVERRIDDEN);
    let sub = health_origin_fixture::guid(SUB);
    let child = catalog_contracts::entry(catalog, &parent, Some(&sub))?;
    let [handler, sub] = requirements(child)? else {
        return Err(
            "inherited handler requirement followed by the authored sub-contract tag".into(),
        );
    };
    check_requirement(
        catalog,
        handler,
        original.handler,
        "handler",
        FAIL_TAG,
        marker,
    )?;
    check_requirement(catalog, sub, original.sub, "subContract", TAG, marker)?;
    check_grants(catalog, sub, original, marker)
}

fn check_repeated(catalog: &Value, original: &Sources, marker: Option<bool>) -> TestResult {
    let row = catalog_contracts::entry(catalog, &health_origin_fixture::guid(REPEATED), None)?;
    let [first, last] = requirements(row)? else {
        return Err("two resolved original occurrences around the omitted dangling slot".into());
    };
    let [first_source, last_source] = original.repeated;
    for (requirement, selected, other) in [
        (first, first_source, last_source),
        (last, last_source, first_source),
    ] {
        let used = check_requirement(catalog, requirement, selected, "contract", TAG, marker)?;
        assert!(
            used.iter()
                .all(|row| !at_site(row, other.tag) && !at_site(row, original.dangling)),
            "equal tags and the dangling slot retain distinct physical identity"
        );
        check_grants(catalog, requirement, original, marker)?;
    }
    Ok(())
}

fn requirements(row: &Value) -> TestResult<&[Value]> {
    origin::rows(origin::cell(row, "requiredIntros", "present")?, "/value")
}

fn check_requirement<'catalog>(
    catalog: &'catalog Value,
    requirement: &Value,
    selected: Requirement,
    level: &str,
    expected: u128,
    marker: Option<bool>,
) -> TestResult<Vec<&'catalog Value>> {
    let label = origin::present(requirement, "level", &json!(level), marker)?;
    let guid = origin::present(
        requirement,
        "tagGuid",
        &json!(health_origin_fixture::guid(expected)),
        marker,
    )?;
    assert!(
        origin::uses(catalog, label)?.iter().any(|row| {
            at_site(row, selected.prerequisite) && has_role(row, "$type", "selector")
        }),
        "the level records the actual completion-tag prerequisite"
    );
    let used = origin::uses(catalog, guid)?;
    assert!(
        used.iter()
            .any(|row| at_site(row, selected.tag) && has_role(row, "$value", "payload")),
        "the tag GUID comes from the exact authored Reference"
    );
    Ok(used)
}

fn check_grants(
    catalog: &Value,
    requirement: &Value,
    original: &Sources,
    marker: Option<bool>,
) -> TestResult {
    let grants = origin::cell(requirement, "introContracts", "present")?;
    assert_eq!(
        grants.get("sourceSetComplete"),
        Some(&json!(true)),
        "readable grant set"
    );
    let [lower, repeated] = origin::rows(grants, "/value")? else {
        return Err("two ordered and distinct logical intro IDs".into());
    };
    check_grant(catalog, lower, LOWER_INTRO, &[&original.lower], marker)?;
    check_grant(
        catalog,
        repeated,
        INTRO,
        &[&original.primary, &original.duplicate],
        marker,
    )?;
    let repeated_uses = origin::uses(catalog, repeated)?;
    assert!(
        repeated_uses.iter().all(|row| {
            !at_site(row, original.lower.id)
                && !at_site(row, original.rejected.id)
                && !at_site(row, original.later_result)
        }),
        "a grant cell stays local to the successful authored witnesses of its ID"
    );
    Ok(())
}

fn check_grant(
    catalog: &Value,
    cell: &Value,
    id: u128,
    grants: &[&Grant],
    marker: Option<bool>,
) -> TestResult {
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "the logical intro is resolved"
    );
    assert_eq!(
        cell.get("value"),
        Some(&json!(health_origin_fixture::guid(id))),
        "actual ID order"
    );
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "actual DCB origin"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "all actual witnesses"
    );
    let used = origin::uses(catalog, cell)?;
    for grant in grants {
        for (site, role) in [
            (grant.id, "payload"),
            (grant.first_result, "selector"),
            (grant.tag, "selector"),
        ] {
            assert!(
                used.iter()
                    .any(|row| at_site(row, site) && has_role(row, "$value", role)),
                "the logical intro retains each physical ID, success gate and tag match"
            );
        }
    }
    Ok(())
}

fn check_no_grant(
    catalog: &Value,
    requirement: &Value,
    original: &Sources,
    marker: Option<bool>,
) -> TestResult {
    let cell = origin::present(requirement, "introContracts", &json!([]), marker)?;
    let used = origin::uses(catalog, cell)?;
    assert!(
        used.iter()
            .any(|row| at_site(row, original.rejected.first_result)
                && has_role(row, "$value", "selector")),
        "known empty grants retain the actual false success gate"
    );
    assert!(
        used.iter().all(|row| !at_site(row, original.later_result) && !at_site(row, original.rejected.tag)),
        "neither a later result nor the failed branch's unvisited tag supplies the empty set"
    );
    Ok(())
}

fn at_site(row: &Value, site: Site) -> bool {
    row.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
        && row.pointer("/location/indices")
            == Some(&json!([site.record.to_string(), site.ordinal.to_string()]))
}

fn has_role(row: &Value, field: &str, role: &str) -> bool {
    row.pointer("/location/field").and_then(Value::as_str) == Some(field)
        && row.get("role").and_then(Value::as_str) == Some(role)
}

fn second_generator() -> TestRecord {
    let repeated = contract(REPEATED, "Repeats").value(
        "additionalPrerequisites",
        TestValue::Array(vec![requires(&[TAG, MISSING_TAG, TAG])]),
    );
    let handler = TestInstance::new("ContractHandler_Career")
        .value("debugName", TestValue::text("OtherCareer"))
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(repeated)]),
        )
        .value(
            "introContracts",
            TestValue::Array(vec![
                intro(INTRO, "DuplicateIntro", &[true], TAG),
                intro(LOWER_INTRO, "LowerIntro", &[true], TAG),
                intro(FAILED_INTRO, "FailureIntro", &[false, true], FAIL_TAG),
            ]),
        );
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.other_facts",
        "libs/foundry/records/contracts/contractgenerator/other_facts.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn intro(id: u128, name: &str, outcomes: &[bool], tag: u128) -> TestValue {
    let flags = outcomes
        .iter()
        .map(|&set| {
            TestValue::Struct(TestInstance::new("Bool").value("value", TestValue::Bool(set)))
        })
        .collect();
    let tag = TestInstance::new("ContractResult_CompletionTag")
        .value("count", TestValue::Int32(1))
        .value("tag", TestValue::Reference(tag));
    let result = TestInstance::new("ContractResult_CompletionTags")
        .value("missionResults", TestValue::Array(flags))
        .value(
            "completionTags",
            TestValue::Array(vec![TestValue::Struct(tag)]),
        );
    TestValue::Struct(contract(id, name).value(
        "contractResults",
        terms(0.0, 0, vec![TestValue::Struct(result)]),
    ))
}

#[path = "facts_required_intros_path.rs"]
mod path;

#[path = "facts_required_intros_join.rs"]
mod join;

#[path = "facts_required_intros_weak.rs"]
mod weak;
