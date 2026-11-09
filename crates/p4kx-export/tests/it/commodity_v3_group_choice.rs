// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, TestResult};
use crate::commodity_v3_group_choice_fixture::{self as group_fixture, FaultPosition};
use crate::commodity_v3_source;
use crate::commodity_v3_source_inventory::{body, header};

#[test]
fn later_unreadable_membership_cannot_taint_an_existing_first_group() -> TestResult {
    let baseline = group_fixture::build(FaultPosition::Absent)?;
    let later = group_fixture::build(FaultPosition::After)?;
    assert_eq!(baseline["exportHealth"]["status"], "complete");
    assert_fault(&later, 3)?;
    let baseline_choice = choice(&baseline)?;
    let later_choice = choice(&later)?;
    for carrier in [baseline_choice, later_choice] {
        assert_eq!(carrier["state"], "present");
        assert_eq!(carrier["value"], fixture::guid(group_fixture::WINNER));
        assert_eq!(
            carrier["sourceSetComplete"], true,
            "a later broken candidate cannot change the established first winner"
        );
        assert_eq!(carrier["containsEncryptedSource"], false);
    }
    let baseline_uses = commodity_v3_source::uses(&baseline, baseline_choice)?;
    let later_uses = commodity_v3_source::uses(&later, later_choice)?;
    assert_eq!(
        baseline_uses, later_uses,
        "the winner's actual decision prefix is independent of later group inputs"
    );
    assert!(later_uses.contains(&(2, header("guid"), "payload".to_owned())));
    assert!(
        later_uses.iter().all(|(index, _, _)| *index != 3),
        "the later unreadable membership supplies no contributor to this choice"
    );
    Ok(())
}

#[test]
fn earlier_unreadable_membership_keeps_the_first_group_choice_incomplete() -> TestResult {
    let catalog = group_fixture::build(FaultPosition::Before)?;
    assert_fault(&catalog, 2)?;
    let carrier = choice(&catalog)?;
    assert_eq!(carrier["state"], "present");
    assert_eq!(
        carrier["value"],
        fixture::guid(group_fixture::WINNER),
        "the established readable choice remains available for diagnosis"
    );
    assert_eq!(
        carrier["sourceSetComplete"], false,
        "an earlier unreadable candidate can hide a different first winner"
    );
    assert!(
        carrier
            .get("containsEncryptedSource")
            .is_some_and(Value::is_null)
    );
    let used = commodity_v3_source::uses(&catalog, carrier)?;
    assert!(
        used.contains(&(2, body("resources"), "selector".to_owned())),
        "the incomplete prefix keeps its actual earlier array selector"
    );
    assert!(
        used.contains(&(3, header("guid"), "payload".to_owned())),
        "the chosen GUID retains its actual physical owner"
    );
    Ok(())
}

fn choice(catalog: &Value) -> TestResult<&Value> {
    fixture::presentation(catalog, fixture::ALPHA)?
        .get("typeGuid")
        .ok_or_else(|| "canonical typeGuid carrier missing".into())
}

fn assert_fault(catalog: &Value, record_index: u64) -> TestResult {
    assert_eq!(
        fixture::at(catalog, "/schemaVersion")?,
        &json!(3),
        "the fixture exercises the v3 producer"
    );
    assert!(
        catalog.get("commodities").is_none(),
        "the canonical producer emits no root legacy rows"
    );
    assert_eq!(
        fixture::at(catalog, "/exportHealth/publicationMode")?,
        &json!("diagnosticPartial"),
        "the requested publication mode is retained"
    );
    assert_eq!(
        fixture::at(catalog, "/exportHealth/status")?,
        &json!("complete"),
        "the represented dangling source state is independent of choice completeness"
    );
    let target = fixture::guid(fixture::DANGLING_RESOURCE);
    let unresolved = fixture::at(catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("exact unresolved source inventory")?;
    assert!(
        unresolved.iter().any(
            |reference| reference.get("recordIndex").and_then(Value::as_u64) == Some(record_index)
                && reference.get("field").and_then(Value::as_str) == Some("resources[0]")
                && reference.get("status").and_then(Value::as_str) == Some("dangling")
                && reference.get("targetGuid").and_then(Value::as_str) == Some(target.as_str())
        ),
        "the fixture exercises a real unavailable source link"
    );
    Ok(())
}
