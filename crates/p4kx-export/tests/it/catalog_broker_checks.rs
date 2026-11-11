// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{OWNER, REWARD, SCOPE};
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::mission_template_graph as mission;
use crate::template_canonical_graph::{BrokerContext, Context};
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;

pub(super) fn reputation_amount(
    graph: &BrokerContext<'_>,
    first: &Value,
    first_source: &Value,
) -> TestResult {
    let amounts = raw::array(first, "amounts")?;
    let [amount] = amounts else {
        return Err("one actual reputation amount".into());
    };
    let source = graph
        .owner()
        .follow(amount.get("source").ok_or("actual original amount entry")?)?;
    let originals = fields::elements(graph, graph.field(first_source, "reputationAmounts")?)?;
    let [original] = originals.as_slice() else {
        return Err("one original authored amount entry".into());
    };
    assert!(
        std::ptr::eq(source, *original),
        "the same actual original first amount"
    );
    fields::reference(graph.field(source, "factionReputation")?, OWNER)?;
    mission::reference(graph.field(source, "reputationScope")?, SCOPE, None)?;
    let reward = graph.bound(
        amount
            .pointer("/reward/field")
            .ok_or("actual reward declaration link")?,
    )?;
    assert!(
        std::ptr::eq(reward, graph.field(source, "reward")?),
        "the actual caller reward reference"
    );
    fields::reference(reward, REWARD)?;
    assert_eq!(
        amount.pointer("/amount/value"),
        Some(&json!("2")),
        "the exact original signed reward amount"
    );
    assert_eq!(
        amount.pointer("/amount/sourceSetComplete"),
        Some(&json!(true)),
        "complete actual reward derivation"
    );
    assert_eq!(
        amount.pointer("/amount/containsEncryptedSource"),
        Some(&json!(true)),
        "actual encrypted reward source"
    );
    assert_eq!(
        amount
            .pointer("/amount/sourceUsesRefs")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(3),
        "the actual caller selector, target payload and target conversion"
    );
    Ok(())
}

pub(super) fn spawns(
    catalog: &Value,
    graph: &BrokerContext<'_>,
    properties: &[&Value],
) -> TestResult {
    let expected = [
        (
            "Wave_BP",
            "ShipSpawnDescriptions",
            "shipSpawnDescriptions",
            "contractShipSpawns",
        ),
        (
            "Place_BP",
            "Location",
            "location",
            "contractLocationQueries",
        ),
        (
            "Crew_BP",
            "NPCSpawnDescriptions",
            "npcSpawnDescriptions",
            "contractNpcSpawns",
        ),
        (
            "Loot_BP",
            "EntitySpawnDescriptions",
            "entitySpawnDescriptions",
            "contractEntitySpawns",
        ),
    ];
    assert_eq!(
        properties.len(),
        expected.len(),
        "all selected original properties in order"
    );
    for (property, (variable, original, meaning, table)) in properties.iter().zip(expected) {
        assert_eq!(
            context::binding_text(graph, property, "variable")?,
            Some(variable),
            "the exact original selected property name"
        );
        let source = context::value(graph, property)?;
        let kind = raw::payload(source, "struct")?
            .pointer("/sourceType/value")
            .and_then(Value::as_str)
            .ok_or("actual original property type")?;
        assert_eq!(
            kind.rsplit('_').next(),
            Some(original),
            "the same original value-type suffix"
        );
        context::spawn(catalog, graph, property, meaning, table)?;
    }
    Ok(())
}
