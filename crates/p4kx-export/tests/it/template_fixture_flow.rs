// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::{Template, condition};
use crate::template_fixture_fields as fields;

const OBJECTIVE_A: &str = "70000000-0000-4000-8000-000000000001";
const OBJECTIVE_B: &str = "70000000-0000-4000-8000-000000000002";
const OBJECTIVE_C: &str = "70000000-0000-4000-8000-000000000003";

fn operation<'catalog>(
    graph: &Template<'catalog>,
    index: usize,
    name: &str,
) -> TestResult<&'catalog Value> {
    let triggers = graph.triggers()?;
    assert_eq!(triggers.len(), 8, "all original triggers in order");
    let trigger = graph
        .owner
        .follow(triggers.get(index).ok_or("actual trigger")?)?;
    graph.owner.follow(
        trigger
            .pointer(&format!("/trigger/{name}"))
            .ok_or("actual operation link")?,
    )
}

fn objective(graph: &Template<'_>, binding: &Value, expected: &str) -> TestResult {
    assert_eq!(
        binding.get("interpretation"),
        Some(&json!("id")),
        "a usable selected original id"
    );
    assert_eq!(
        fields::scalar(
            graph.bound(binding.get("field").ok_or("selected id field")?)?,
            "guid"
        )?,
        &json!(expected),
        "exact physical objective target id"
    );
    Ok(())
}

pub(crate) fn structure(graph: &Template<'_>) -> TestResult {
    let first_trigger = graph
        .owner
        .follow(graph.triggers()?.first().ok_or("first original trigger")?)?;
    assert_eq!(
        fields::text(graph.field(first_trigger, "description")?)?,
        "All done -> Completed",
        "the exact original note"
    );
    first_condition(graph)?;
    let action = operation(graph, 0, "action")?;
    assert_eq!(
        action.pointer("/action/type"),
        Some(&json!("stateChange")),
        "the original action kind"
    );
    assert_eq!(
        action.pointer("/action/state"),
        Some(&json!({"type":"known","state":"completed"})),
        "the established state-change interpretation"
    );
    random(graph)?;
    for (index, kind, state) in [
        (2, "allTokensState", Some("completed")),
        (6, "anyTokensState", Some("failed")),
        (7, "onMissionStart", None),
    ] {
        let node = operation(graph, index, "condition")?;
        assert_eq!(
            condition(node)?.get("type"),
            Some(&json!(kind)),
            "the exact condition kind"
        );
        if let Some(state) = state {
            assert_eq!(
                condition(node)?.get("state"),
                Some(&json!({"type":"known","state":state})),
                "the established contextual token state"
            );
        }
    }
    let unknown = operation(graph, 2, "action")?;
    assert_eq!(
        fields::text(graph.field(unknown, "toState")?)?,
        "Sleeping",
        "the unknown original label stays"
    );
    assert_eq!(
        unknown.pointer("/action/state"),
        Some(&json!({"type":"unknown"})),
        "unsupported vocabulary has an explicit separate interpretation"
    );
    Ok(())
}

fn first_condition(graph: &Template<'_>) -> TestResult {
    let root = operation(graph, 0, "condition")?;
    assert_eq!(
        condition(root)?.get("type"),
        Some(&json!("and")),
        "AND remains an explicit operation"
    );
    let children = raw::array(condition(root)?, "children")?;
    assert_eq!(children.len(), 2, "the actual ordered AND members");
    let first = graph
        .owner
        .follow(children.first().ok_or("first AND member")?)?;
    token(condition(first)?, "completed");
    objective(
        graph,
        condition(first)?
            .get("objective")
            .ok_or("actual objective binding")?,
        OBJECTIVE_A,
    )?;
    let not = graph
        .owner
        .follow(children.get(1).ok_or("second AND member")?)?;
    assert_eq!(
        condition(not)?.get("type"),
        Some(&json!("not")),
        "the explicit NOT operation"
    );
    let nested = graph
        .owner
        .follow(condition(not)?.get("child").ok_or("actual NOT child")?)?;
    token(condition(nested)?, "failed");
    objective(
        graph,
        condition(nested)?.get("objective").ok_or("nested target")?,
        OBJECTIVE_B,
    )?;
    Ok(())
}

fn token(meaning: &Value, expected: &str) {
    assert_eq!(
        meaning.get("type"),
        Some(&json!("tokenState")),
        "the original token condition kind"
    );
    assert_eq!(
        meaning.get("state"),
        Some(&json!({"type":"known","state":expected})),
        "known token state"
    );
}

fn random(graph: &Template<'_>) -> TestResult {
    let action = operation(graph, 1, "action")?;
    assert_eq!(
        action.pointer("/action/type"),
        Some(&json!("pickRandomMissionPhase")),
        "random action"
    );
    assert_eq!(
        action.pointer("/action/state"),
        Some(&json!({"type":"known","state":"active"})),
        "target state"
    );
    let phases = raw::array(
        action.get("action").ok_or("actual action meaning")?,
        "phases",
    )?;
    assert_eq!(phases.len(), 1, "the original selected random phase");
    let phase = graph
        .owner
        .follow(phases.first().ok_or("actual random phase")?)?;
    objective(
        graph,
        phase
            .pointer("/randomPhase/objective")
            .ok_or("actual phase target")?,
        OBJECTIVE_C,
    )?;
    assert_eq!(
        phase.pointer("/randomPhase/probabilityInterpretation"),
        Some(&json!("finite")),
        "the exact finite probability normalization recipe"
    );
    let probability = graph.field(phase, "probability")?;
    let encoded = fields::scalar(probability, "number")?;
    assert_eq!(
        encoded.get("type"),
        Some(&json!("f32")),
        "the exact original Single width"
    );
    assert_eq!(
        encoded.get("rawBits"),
        Some(&json!("3e800000")),
        "the exact original Single bits"
    );
    assert_eq!(
        fields::number(probability)?,
        &json!(0.25),
        "original Single probability"
    );
    let null = operation(graph, 1, "condition")?;
    token(condition(null)?, "active");
    assert_eq!(
        condition(null)?.pointer("/objective/interpretation"),
        Some(&json!("null")),
        "the actual unset weak slot is null"
    );
    Ok(())
}

pub(crate) fn bindings(graph: &Template<'_>) -> TestResult {
    let nil = operation(graph, 3, "condition")?;
    let binding = condition(nil)?
        .get("objective")
        .ok_or("nil objective binding")?;
    assert_eq!(
        binding.get("interpretation"),
        Some(&json!("nil")),
        "genuine nil is distinct from null"
    );
    assert_eq!(
        fields::scalar(
            graph.bound(binding.get("field").ok_or("real nil id field")?)?,
            "guid"
        )?,
        &json!("00000000-0000-0000-0000-000000000000"),
        "the actual authored nil id remains"
    );
    let property = operation(graph, 4, "condition")?;
    assert_eq!(
        condition(property)?.get("type"),
        Some(&json!("propertyIsSet")),
        "the property condition"
    );
    let binding = condition(property)?
        .get("property")
        .ok_or("actual property binding")?;
    assert_eq!(
        binding.get("interpretation"),
        Some(&json!("value")),
        "usable original variable"
    );
    assert_eq!(
        fields::text(graph.bound(binding.get("field").ok_or("actual variable field")?)?)?,
        "Place_BP",
        "the real selected property variable"
    );
    let outside = operation(graph, 5, "condition")?;
    assert_eq!(
        condition(outside)?.pointer("/objective/interpretation"),
        Some(&json!("outsideOwner")),
        "the genuine foreign instance stays distinct from nil or null"
    );
    Ok(())
}
