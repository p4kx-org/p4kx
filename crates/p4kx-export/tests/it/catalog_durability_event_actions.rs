// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_durability_event_actions_fixture.rs"]
mod fixture;

const DURABILITY_ROW: &str = "/configurations/durability/rows/0";

#[test]
fn durability_event_actions_keeps_authored_maelstrom_slots_and_links() -> TestResult {
    let catalog = catalog(&fixture::database())?;
    let row = proof::at(&catalog, DURABILITY_ROW)?;
    let [base, destroy, state, null] = proof::rows(row, "/maelstromEvents/value")? else {
        return Err("four original Maelstrom slots".into());
    };
    assert_eq!(
        proof::at(base, "/value/type")?,
        "base",
        "original base form"
    );
    assert_eq!(
        proof::at(destroy, "/value/type")?,
        "destroy",
        "distinct authored event"
    );
    assert_eq!(
        proof::at(state, "/value/type")?,
        "interactionStateMachine",
        "original interaction-state form"
    );
    assert_eq!(proof::at(null, "/state")?, "null", "explicit null slot");
    let base = proof::at(base, "/value/value")?;
    let destroy = proof::at(destroy, "/value/value")?;
    check_tags(&catalog, base, destroy)?;
    let state = proof::at(state, "/value/value")?;
    check_state(state)
}

#[test]
fn durability_event_actions_keeps_duplicate_task_paths_and_exact_uses() -> TestResult {
    let catalog = catalog(&fixture::database())?;
    let row = proof::at(&catalog, DURABILITY_ROW)?;
    let [first, base, last] = proof::rows(row, "/sequencerTasks/value")? else {
        return Err("three original sequencer slots".into());
    };
    let first = check_task(&catalog, first)?;
    assert_eq!(proof::at(base, "/value/type")?, "base", "fieldless task");
    let last = check_task(&catalog, last)?;
    assert_ne!(
        first, last,
        "duplicate stored paths keep different original value coordinates"
    );
    proof::closure(&catalog)
}

#[test]
fn durability_event_actions_distinguishes_empty_and_unsupported_arrays() -> TestResult {
    let catalog = catalog(&fixture::unsupported_database())?;
    let row = proof::at(&catalog, DURABILITY_ROW)?;
    proof::present(row, "maelstromEvents", &json!([]), Some(true))?;
    let [unknown, valid] = proof::rows(row, "/sequencerTasks/value")? else {
        return Err("unsupported task and original next slot".into());
    };
    assert_eq!(proof::at(unknown, "/state")?, "unsupported", "unknown task");
    assert_eq!(
        proof::at(unknown, "/value")?,
        &Value::Null,
        "no invented task"
    );
    check_task(&catalog, valid)?;
    Ok(())
}

fn catalog(source: &DataForgeBuilder) -> TestResult<Value> {
    let (_folder, archive) = common::open(source, DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn check_tags(catalog: &Value, base: &Value, destroy: &Value) -> TestResult {
    let base_link = proof::at(base, "/eventTag/value/value")?;
    let repeated = proof::at(destroy, "/eventTag/value/value")?;
    let expected = json!(common::guid(fixture::TAG));
    proof::present(base_link, "guid", &expected, Some(true))?;
    proof::present(repeated, "guid", &expected, Some(true))?;
    proof::cell(base, "groupTag", "null")?;
    let group = proof::at(destroy, "/groupTag/value/value")?;
    assert_eq!(
        proof::at(group, "/status")?,
        "dangling",
        "the authored unresolved group link"
    );
    let guid = proof::present(
        group,
        "guid",
        &json!(common::guid(fixture::MISSING_TAG)),
        Some(true),
    )?;
    assert!(
        proof::uses(catalog, guid)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("$error.guid")
        }),
        "unresolved group GUID retains the original Reference parameter"
    );
    Ok(())
}

fn check_state(event: &Value) -> TestResult {
    let link = proof::at(event, "/state/value")?;
    assert_eq!(proof::at(link, "/type")?, "weakReference", "actual pointer");
    let target = proof::at(link, "/value/target")?;
    assert_eq!(
        proof::at(target, "/type")?,
        "instance",
        "actual owned state"
    );
    proof::present(
        target,
        "location",
        &json!("Components[1].fixtureLocalState"),
        Some(true),
    )?;
    proof::present(
        target,
        "sourceType",
        &json!("SInteractionState"),
        Some(true),
    )?;
    Ok(())
}

fn check_task(catalog: &Value, task: &Value) -> TestResult<Value> {
    assert_eq!(
        proof::at(task, "/value/type")?,
        "applyDamageMap",
        "the concrete authored task"
    );
    let path = proof::present(
        proof::at(task, "/value/value")?,
        "damageMapPath",
        &json!(fixture::DAMAGE_PATH),
        Some(true),
    )?;
    let uses = proof::uses(catalog, path)?;
    let payload = uses.iter().find(|used| {
        used.get("role").and_then(Value::as_str) == Some("payload")
            && used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
            && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
    });
    let payload = payload.ok_or("physical sequencer path value")?;
    assert_eq!(
        proof::at(payload, "/location/indices/0")?,
        "0",
        "item owner"
    );
    Ok(proof::at(payload, "/location/indices/1")?.clone())
}
