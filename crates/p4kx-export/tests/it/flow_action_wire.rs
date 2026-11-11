// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;

pub(super) fn trigger<'catalog>(
    graph: &Template<'catalog>,
    index: usize,
) -> TestResult<&'catalog Value> {
    let triggers = graph.triggers()?;
    assert_eq!(
        triggers.len(),
        3,
        "all three original trigger Struct occurrences remain"
    );
    graph
        .owner
        .follow(triggers.get(index).ok_or("actual trigger occurrence")?)
}

pub(super) fn operation<'catalog>(
    graph: &Template<'catalog>,
    index: usize,
    operation: &str,
) -> TestResult<&'catalog Value> {
    let trigger = trigger(graph, index)?;
    graph.owner.follow(
        trigger
            .pointer(&format!("/trigger/{operation}"))
            .ok_or("actual trigger operation link")?,
    )
}

pub(super) fn objective<'catalog>(
    graph: &Template<'catalog>,
    node: &Value,
    operation: &str,
) -> TestResult<&'catalog Value> {
    let binding = node
        .pointer(&format!("/{operation}/objective"))
        .ok_or("actual objective binding")?;
    assert_eq!(
        binding.get("interpretation"),
        Some(&json!("id")),
        "the selected genuine target contains a usable authored id"
    );
    Ok(raw::payload(
        graph.bound(binding.get("field").ok_or("actual target field link")?)?,
        "guid",
    )?
    .get("value")
    .ok_or("exact selected original GUID")?)
}

pub(super) fn phases<'catalog>(graph: &Template<'catalog>) -> TestResult<Vec<&'catalog Value>> {
    let action = operation(graph, 2, "action")?;
    let selected = raw::array(
        action.get("action").ok_or("actual ActionMeaning")?,
        "phases",
    )?;
    assert_eq!(
        selected.len(),
        2,
        "the real null phase remains raw, separate from existing Struct selection"
    );
    let original = raw::elements(graph.field(action, "missionPhases")?)?;
    assert_eq!(
        original.len(),
        3,
        "both real Structs and their actual intervening null survive"
    );
    raw::source_null(
        graph
            .owner
            .follow(original.get(1).ok_or("original null phase slot")?)?,
    );
    selected
        .iter()
        .map(|reference| graph.owner.follow(reference))
        .collect()
}

pub(super) fn probability(graph: &Template<'_>, node: &Value, expected: f32) -> TestResult<f64> {
    let meaning = node.get("randomPhase").ok_or("actual RandomPhaseMeaning")?;
    assert_eq!(
        meaning.get("probabilityInterpretation"),
        Some(&json!("finite")),
        "the retained explicit recipe is finite-f32 normalization"
    );
    let number = raw::payload(graph.field(node, "probability")?, "number")?
        .get("value")
        .ok_or("exact original numeric encoding")?;
    assert_eq!(
        number.get("type"),
        Some(&json!("f64")),
        "the original Double stays a Double"
    );
    let original = number
        .get("finite")
        .and_then(Value::as_f64)
        .ok_or("actual finite original probability")?;
    assert!(
        expected.is_finite(),
        "the actual f32 probability recipe is representable"
    );
    let projected = f64::from(expected);
    let lower = f64::midpoint(projected, f64::from(expected.next_down()));
    let upper = f64::midpoint(projected, f64::from(expected.next_up()));
    assert!(
        original > lower && original < upper,
        "the original Double has exactly this unique nearest finite-f32 normalization"
    );
    Ok(projected)
}
