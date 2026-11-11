// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Broker properties, spawn pools, orders, objectives, and modifiers.

use serde_json::{Value, json};

use crate::broker_flow_graph as broker;
use crate::catalog_contract_templates::build;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::{BrokerContext, Context};
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;

use super::{BROKER, ELSEWHERE, OWNER, checks};

#[test]
fn properties_spawns_and_orders_are_read_like_those_of_a_template() -> TestResult {
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    let properties = context::properties(&graph, None)?;
    checks::spawns(&catalog, &graph, &properties)?;
    let place = properties.get(1).ok_or("actual second property")?;
    assert_eq!(
        context::binding_text(&graph, place, "token")?,
        Some("Place"),
        "the complete original token"
    );
    for (family, variable) in [("ships", "Wave_BP"), ("location", "Place_BP")] {
        let pools = context::pools(&graph, family)?;
        let [pool] = pools.as_slice() else {
            return Err("one actual original selected Broker pool".into());
        };
        assert_eq!(
            (pool.variable, pool.level),
            (variable, "broker"),
            "the same original selection and Broker level"
        );
    }
    hauling(&graph)
}

fn hauling(graph: &BrokerContext<'_>) -> TestResult {
    let orders = raw::array(graph.projection(), "haulingOrders")?;
    let [order] = orders else {
        return Err("one actual original Broker hauling order".into());
    };
    assert_eq!(
        order.get("source"),
        Some(&json!("brokerObjective")),
        "the same selected objective source"
    );
    let item = graph.owner().follow(
        order
            .pointer("/requirements/0/options/0/0/source")
            .ok_or("actual grouped item position")?,
    )?;
    assert_eq!(
        fields::number(graph.field(item, "maxSCU")?)?,
        &json!(8),
        "the exact original grouped quantity"
    );
    Ok(())
}

#[test]
fn objectives_payout_flow_and_modifiers_use_the_shapes_of_the_template() -> TestResult {
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    assert_eq!(
        context::objectives(&graph)?.len(),
        1,
        "one actual original selected objective"
    );
    let objective = context::objective(&graph, 0)?;
    assert_eq!(
        fields::text(graph.field(objective, "debugName")?)?,
        "Haul",
        "the exact original objective name"
    );
    let handler = fields::structure(&graph, graph.field(objective, "objectiveHandler")?)?;
    raw::structure(handler, "ObjectiveHandler_Hauling")?;
    let payout = fields::structure(&graph, graph.root("partialRewardPayout")?)?;
    let thresholds = fields::elements(&graph, graph.field(payout, "percentageThresholds")?)?;
    let [threshold] = thresholds.as_slice() else {
        return Err("one actual original payout threshold".into());
    };
    fields::numeric_fields(&graph, threshold, &[("minPercentage", json!(76))])?;
    let id = fields::scalar(graph.field(objective, "id")?, "guid")?;
    broker::token_state(&catalog, BROKER, id)?;
    modifiers(&graph)
}

fn modifiers(graph: &BrokerContext<'_>) -> TestResult {
    let rows = raw::array(graph.projection(), "modifiers")?;
    let originals = fields::elements(graph, graph.root("modifiers")?)?;
    let kinds = [
        "factionHostility",
        "hostileMission",
        "ignoreMissionPlayerCriminality",
        "lawLicense",
    ];
    assert_eq!(
        rows.len(),
        kinds.len(),
        "all four actual selected modifiers"
    );
    assert_eq!(
        originals.len(),
        kinds.len(),
        "all four original modifier slots"
    );
    for ((row, original), kind) in rows.iter().zip(&originals).zip(kinds) {
        let node = graph
            .owner()
            .follow(row.get("source").ok_or("actual original modifier link")?)?;
        assert!(
            std::ptr::eq(node, *original),
            "the same actual original modifier ordering"
        );
        assert_eq!(
            row.pointer("/kind/value"),
            Some(&json!(kind)),
            "the same recognized modifier"
        );
        fields::scalar_fields(graph, node, &[("enabled", "boolean", json!(true))])?;
        fields::absent_field(node, "locationProperty")?;
    }
    modifier_details(graph, &originals)
}

fn modifier_details(graph: &BrokerContext<'_>, originals: &[&Value]) -> TestResult {
    let [faction, hostile, ignore, law] = originals else {
        return Err("four exact original modifier occurrences".into());
    };
    assert_eq!(
        fields::text(graph.field(faction, "modifierName")?)?,
        "Rivals",
        "the exact original name"
    );
    fields::reference(graph.field(faction, "faction")?, OWNER)?;
    super::vocabulary(
        graph,
        graph.field(faction, "myReaction")?,
        "Hostile",
        &json!("hostile"),
    )?;
    super::vocabulary(
        graph,
        graph.field(faction, "theirReaction")?,
        "Gloating",
        &Value::Null,
    )?;
    fields::scalar_fields(
        graph,
        faction,
        &[("ignoreCriminalHostility", "boolean", json!(true))],
    )?;
    for node in [hostile, ignore] {
        fields::reference(graph.field(node, "missionBrokerEntry")?, ELSEWHERE)?;
    }
    fields::scalar_fields(graph, hostile, &[("legalToAttack", "boolean", json!(true))])?;
    for node in [hostile, ignore, law] {
        fields::absent_field(node, "modifierName")?;
    }
    super::vocabulary(
        graph,
        graph.field(law, "licenseType")?,
        "IllegalGoodsLicense_ClassB",
        &json!("illegalGoodsLicenseClassB"),
    )
}
