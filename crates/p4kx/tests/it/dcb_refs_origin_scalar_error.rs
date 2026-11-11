// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, ReferenceIndex, Value, ValueError};
use p4kx_testkit::{TestRecord, TestStruct, TestType, TestValue};
use serde_json::json;

use super::fixture::{Case, Fixture, Input, Site, TestResult, array, site, value};

const MIXED_PATH: &str = "libs/ships/mixed";
const MIXED_GUID: u128 = 0x45;

#[test]
fn an_unrelated_scalar_error_keeps_same_record_and_global_reference_counts_complete() -> TestResult
{
    let fixture = Fixture::open(&mixed_database(), &Input::ALL, assert_mixed_original)?;
    for case in &fixture.inputs {
        check_local_counts(case)?;
        check_global_counts(case)?;
    }
    Ok(())
}

fn check_local_counts(case: &Case<Site>) -> TestResult {
    let document = case.run(&[MIXED_PATH, "--outgoing", "--json"])?;
    let [row] = array(&document, "records")? else {
        return Err("one selected Record with a scalar error and a reference".into());
    };
    for field in ["reference_count", "record_count"] {
        assert_eq!(
            row.get(field),
            Some(&json!(1)),
            "the readable local {field}"
        );
        let uses = case.uses(&document, row, &format!("/{field}"))?;
        value(&uses, case.original, "$value", "selector");
    }
    assert_eq!(
        row.get("unresolved_count"),
        Some(&json!(0)),
        "an invalid string is not an unresolved reference"
    );
    case.uses(&document, row, "/unresolved_count")?;
    Ok(())
}

fn check_global_counts(case: &Case<Site>) -> TestResult {
    let document = case.run(&["libs/ships/beta", "--incoming", "--json"])?;
    let [_, row] = array(&document, "records")? else {
        return Err("two actual same-path incoming rows".into());
    };
    for field in ["reference_count", "record_count"] {
        assert_eq!(
            row.get(field),
            Some(&json!(1)),
            "the readable incoming {field}"
        );
        let uses = case.uses(&document, row, &format!("/{field}"))?;
        value(&uses, case.original, "$value", "selector");
    }
    let unresolved = case.run(&["--unresolved", "--json"])?;
    assert_eq!(
        unresolved.get("count"),
        Some(&json!(2)),
        "only the two original dangling References are counted"
    );
    case.uses(&unresolved, &unresolved, "/count")?;
    Ok(())
}

fn mixed_database() -> Vec<u8> {
    super::super::original_database()
        .structure(
            TestStruct::new("MixedReferenceHolder")
                .field("label", TestType::String)
                .field("subject", super::super::reference_to("Ship")),
        )
        .record(
            TestRecord::new(
                MIXED_GUID,
                "MixedReferenceHolder",
                "MixedReferenceHolder.Control",
                "libs/ships/mixed.xml",
            )
            .value("label", TestValue::Raw(u32::MAX.to_le_bytes().to_vec()))
            .value("subject", TestValue::Reference(super::super::GAMMA)),
        )
        .build()
}

fn assert_mixed_original(forge: &DataForge<'_>) -> TestResult<Site> {
    let holder = forge.record(5).ok_or("actual mixed holder Record")?;
    let tree = holder.tree()?;
    assert!(
        tree.is_from(&holder),
        "both fields belong to this same physical Record"
    );
    assert_eq!(tree.error_count(), 1, "only the unrelated scalar fails");
    assert!(
        matches!(
            tree.field("label"),
            Some(Value::Error(ValueError::InvalidString {
                offset: u32::MAX,
                ..
            }))
        ),
        "the original scalar is definitely an invalid string"
    );
    let subject = tree.field("subject").ok_or("actual original subject")?;
    let Value::Reference(target) = subject else {
        return Err("the original subject still resolves".into());
    };
    let gamma = forge.record(4).ok_or("actual same-path target Gamma")?;
    assert_eq!(
        target.original_target().record()?.index(),
        gamma.index(),
        "the decoder selects the exact same-path target"
    );
    let index = ReferenceIndex::build(forge);
    assert_eq!(index.unreadable_records(), 0, "all original trees build");
    assert_eq!(
        index.incoming(gamma.index()).len(),
        1,
        "one original incoming occurrence"
    );
    assert_eq!(
        index.outgoing(holder.index()).len(),
        1,
        "one original outgoing occurrence"
    );
    site(&tree, subject)
}
