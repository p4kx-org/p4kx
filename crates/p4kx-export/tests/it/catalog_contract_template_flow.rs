// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Template mission flow, property pointers, and start and end communications.

use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, canonical};
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;
use crate::template_fixture_tags as tags;

#[test]
fn the_mission_flow_keeps_flat_conditions_and_explicit_unknown_states() -> TestResult {
    let catalog = build()?;
    crate::template_fixture_flow::structure(&canonical(&catalog)?)
}

#[test]
fn a_weak_pointer_names_the_objective_or_the_property_or_has_an_explicit_absence() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    crate::template_fixture_flow::bindings(&graph)?;
    let modifiers = raw::array(graph.projection, "modifiers")?;
    let modifier = graph.owner.follow(
        modifiers
            .first()
            .and_then(|row| row.get("source"))
            .ok_or("actual first modifier source")?,
    )?;
    context::property_target(
        &graph,
        graph.field(modifier, "locationProperty")?,
        "Home_BP",
    )?;
    context::property_target(
        &graph,
        graph.field(super::handler(&graph, 2)?, "location")?,
        "Place_BP",
    )?;
    Ok(())
}

#[test]
fn the_start_and_end_comms_name_their_tags_and_reasons() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let start = fields::structure(&graph, fields::required_input(&graph, "startComms")?)?;
    fields::numeric_fields(&graph, start, &[("delay", json!(3))])?;
    tags::path(
        &graph,
        graph.field(start, "communicationTags")?,
        Some("Kill"),
    )?;
    let end = fields::elements(&graph, fields::required_input(&graph, "endComms")?)?;
    assert_eq!(end.len(), 2, "all original ending notifications in order");
    for (row, original, meaning, delay, tag) in [
        (
            *end.first().ok_or("first end notification")?,
            "Complete",
            json!("complete"),
            5,
            Some("Kill"),
        ),
        (
            *end.get(1).ok_or("second end notification")?,
            "Vanish",
            Value::Null,
            0,
            None,
        ),
    ] {
        let reason = graph.field(row, "reason")?;
        assert_eq!(
            fields::text(reason)?,
            original,
            "even an unknown authored label survives"
        );
        assert_eq!(
            fields::vocabulary(&graph, reason)?.get("value"),
            Some(&meaning),
            "separate reason interpretation"
        );
        fields::numeric_fields(&graph, row, &[("delay", json!(delay))])?;
        tags::path(&graph, graph.field(row, "communicationTags")?, tag)?;
    }
    Ok(())
}
