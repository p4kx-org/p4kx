// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical contract-level source values and their separate contextual interpretations.

use serde_json::json;

use crate::catalog_contract_templates::{build, canonical};
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;
use crate::template_fixture_text as texts;

const OWNER: &str = "80000000-0000-4000-8000-000000000001";

#[test]
fn spawns_and_locations_link_to_the_shared_kinds_like_those_of_an_objective() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    for (variable, kind, target) in [
        ("Wave_BP", "shipSpawnDescriptions", "contractShipSpawns"),
        ("Place_BP", "location", "contractLocationQueries"),
        ("Guard_BP", "npcSpawnDescriptions", "contractNpcSpawns"),
        ("Loot_BP", "entitySpawnDescriptions", "contractEntitySpawns"),
    ] {
        context::spawn(
            &catalog,
            &graph,
            context::named(&graph, variable)?,
            kind,
            target,
        )?;
    }
    Ok(())
}

#[test]
fn a_boolean_and_hauling_orders_state_their_content() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let flag = context::named(&graph, "Flag_BP")?;
    context::kind(&graph, flag, "boolean")?;
    fields::scalar_fields(
        &graph,
        context::value(&graph, flag)?,
        &[("value", "boolean", json!(true))],
    )?;
    let cargo = context::named(&graph, "Cargo_BP")?;
    context::kind(&graph, cargo, "haulingOrders")?;
    let source = cargo.get("source").ok_or("actual cargo source")?;
    let matching: Vec<_> = raw::array(graph.projection, "haulingOrders")?
        .iter()
        .filter(|row| row.get("property") == Some(source))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the actual hauling property has one contextual order"
    );
    let item = matching
        .first()
        .and_then(|row| row.pointer("/requirements/0/options/0/0"))
        .ok_or("actual selected hauling item")?;
    assert_eq!(
        item.get("kind"),
        Some(&json!("resource")),
        "the actual item interpretation"
    );
    let original = graph
        .owner
        .follow(item.get("source").ok_or("actual original hauling source")?)?;
    fields::numeric_fields(&graph, original, &[("maxSCU", json!(4))])?;
    Ok(())
}

#[test]
fn a_mission_item_states_its_conditions_and_limits() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let property = context::named(&graph, "Item_BP")?;
    context::kind(&graph, property, "missionItem")?;
    let item = context::value(&graph, property)?;
    fields::reference(graph.field(item, "debugForceChosenItem")?, OWNER)?;
    fields::numeric_fields(
        &graph,
        item,
        &[("minItemsToFind", json!(1)), ("maxItemsToFind", json!(3))],
    )?;
    let conditions = fields::elements(&graph, graph.field(item, "matchConditions")?)?;
    assert_eq!(
        conditions.len(),
        2,
        "both actual conditions in original order"
    );
    let source = property.get("value").ok_or("actual item source")?;
    let meaning = raw::array(graph.projection, "valueMeanings")?
        .iter()
        .find(|row| row.get("source") == Some(source))
        .ok_or("actual item meaning")?;
    for (row, expected) in raw::array(meaning, "conditions")?
        .iter()
        .zip(["tagSearch", "specificItems"])
    {
        assert_eq!(
            raw::present(row.get("kind").ok_or("actual condition kind")?)?,
            &json!(expected),
            "the recognized original condition family"
        );
    }
    assert_eq!(
        raw::array(meaning, "conditions")?.len(),
        2,
        "no contextual condition disappears"
    );
    let specific = *conditions.get(1).ok_or("actual specific-items condition")?;
    let items = fields::elements(&graph, graph.field(specific, "items")?)?;
    assert_eq!(items.len(), 1, "the original selected item list");
    fields::reference(items.first().ok_or("actual specific item")?, OWNER)?;
    Ok(())
}

#[test]
fn a_name_states_its_texts_and_its_name_data() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let property = context::named(&graph, "Name_BP")?;
    context::kind(&graph, property, "aiName")?;
    let name = context::value(&graph, property)?;
    fields::scalar_fields(
        &graph,
        name,
        &[
            ("randomName", "boolean", json!(true)),
            ("randomLastName", "boolean", json!(false)),
            ("randomNickName", "boolean", json!(true)),
        ],
    )?;
    texts::localized(
        &catalog,
        &graph,
        graph.field(name, "characterGivenName")?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    for (field, original) in [
        ("characterGivenLastName", "@LOC_UNINITIALIZED"),
        ("characterGivenNickName", "@LOC_EMPTY"),
    ] {
        texts::no_lookup(&graph, graph.field(name, field)?, original, "placeholder")?;
    }
    fields::numeric_fields(&graph, name, &[("chanceOfNickName", json!(0.25))])?;
    fields::reference(graph.field(name, "characterNameData")?, OWNER)?;
    Ok(())
}

#[test]
fn combined_entries_a_reward_and_a_race_state_their_fields() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let combined = context::named(&graph, "Combined_BP")?;
    context::kind(&graph, combined, "combinedDataSetEntries")?;
    let targets = fields::elements(
        &graph,
        graph.field(context::value(&graph, combined)?, "dataSetEntryProperties")?,
    )?;
    assert_eq!(
        targets.len(),
        2,
        "both real original property pointers remain ordered"
    );
    for (target, variable) in targets.iter().zip(["Place_BP", "Home_BP"]) {
        context::property_target(&graph, target, variable)?;
    }
    let pay = context::named(&graph, "Pay_BP")?;
    context::kind(&graph, pay, "reward")?;
    let reward = graph.field(context::value(&graph, pay)?, "rewardDef")?;
    assert_eq!(
        fields::text(graph.field(reward, "currencyType")?)?,
        "UEC",
        "the actual currency"
    );
    fields::numeric_fields(
        &graph,
        reward,
        &[("reward", json!(900)), ("max", json!(1200))],
    )?;
    fields::scalar_fields(&graph, reward, &[("plusBonuses", "boolean", json!(true))])?;
    fields::reference(graph.field(reward, "reputationBonus")?, OWNER)?;
    for (variable, kind) in [("Race_BP", "timeTrialRace"), ("Map_BP", "starMapLocation")] {
        let property = context::named(&graph, variable)?;
        context::kind(&graph, property, kind)?;
        let node = context::value(&graph, property)?;
        if variable == "Race_BP" {
            assert!(
                raw::elements(graph.field(node, "targetSplits")?)?.is_empty(),
                "the actual empty race list"
            );
        } else {
            assert!(
                raw::array(raw::payload(node, "struct")?, "fields")?.is_empty(),
                "the actual unit source Struct"
            );
        }
    }
    Ok(())
}
