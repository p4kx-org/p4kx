// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Canonical contract-level source values and their separate contextual interpretations.

use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, canonical};
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::{BrokerContext, Context};
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
    let template = canonical(&catalog)?;
    let broker = BrokerContext::new(
        &catalog,
        &crate::commodity_v3_fixture::guid(crate::spawns_fixture::BROKER),
    )?;
    verify_item(&template)?;
    verify_item(&broker)
}

fn verify_item(graph: &dyn Context<'_>) -> TestResult {
    let property = context::named(graph, "Item_BP")?;
    context::kind(graph, property, "missionItem")?;
    let item = context::value(graph, property)?;
    let meaning = raw::array(graph.projection(), "valueMeanings")?
        .iter()
        .find(|row| row.get("source") == property.get("value"))
        .ok_or("one contextual value meaning for the original item property")?;
    let selection = meaning
        .get("missionItem")
        .ok_or("MissionItem field bindings")?;
    for field in [
        "minItemsToFind",
        "maxItemsToFind",
        "debugForceChosenItem",
        "matchConditions",
    ] {
        assert!(
            std::ptr::eq(bound(graph, selection, field)?, graph.field(item, field)?),
            "each field binding leads to the original cell in this owner"
        );
    }
    fields::reference(bound(graph, selection, "debugForceChosenItem")?, OWNER)?;
    fields::numeric_fields(
        graph,
        item,
        &[("minItemsToFind", json!(1)), ("maxItemsToFind", json!(3))],
    )?;
    verify_conditions(graph, meaning)
}

fn verify_conditions(graph: &dyn Context<'_>, meaning: &Value) -> TestResult {
    let [tag, specific] = raw::array(meaning, "conditions")? else {
        return Err("both original conditions retained in order".into());
    };
    for (row, expected) in [(tag, "tagSearch"), (specific, "specificItems")] {
        assert_eq!(
            raw::present(row.get("kind").ok_or("actual condition kind")?)?,
            &json!(expected),
            "the original condition kind follows its own source"
        );
    }
    assert!(
        tag.get("specificItems").is_some_and(Value::is_null),
        "tag matching has no explicit item list"
    );
    verify_candidates(graph, specific)
}

fn verify_candidates(graph: &dyn Context<'_>, specific: &Value) -> TestResult {
    let original = graph
        .owner()
        .follow(specific.get("source").ok_or("condition source")?)?;
    let navigation = specific
        .get("specificItems")
        .ok_or("explicit candidate binding")?;
    let items = bound(graph, navigation, "items")?;
    assert!(
        std::ptr::eq(items, graph.field(original, "items")?),
        "the list binding resolves to the condition's original items array"
    );
    let candidates = raw::array(navigation, "candidates")?;
    assert_eq!(
        candidates,
        raw::elements(items)?,
        "every slot is navigable in original order"
    );
    let [first, null, repeated, last] = candidates else {
        return Err("four original candidate slots".into());
    };
    fields::reference(graph.owner().follow(first)?, OWNER)?;
    raw::source_null(graph.owner().follow(null)?);
    fields::reference(graph.owner().follow(repeated)?, OWNER)?;
    let unreadable = graph.owner().follow(last)?;
    assert_eq!(
        unreadable.pointer("/value/state"),
        Some(&json!("unreadable")),
        "the dangling candidate retains its original unreadable cell"
    );
    assert_eq!(
        unreadable.pointer("/originalError/type"),
        Some(&json!("danglingReference")),
        "the original reference failure remains available"
    );
    let guid = unreadable
        .pointer("/originalError/guid")
        .ok_or("original dangling candidate GUID")?;
    assert_eq!(
        raw::present(guid)?,
        &json!(crate::commodity_v3_fixture::guid(
            crate::spawns_fixture::RESOURCE
        )),
        "the unreadable candidate keeps its authored target"
    );
    assert_ne!(
        first, repeated,
        "same target still has two original physical cells"
    );
    Ok(())
}

fn bound<'catalog>(
    graph: &dyn Context<'catalog>,
    bindings: &Value,
    name: &str,
) -> TestResult<&'catalog Value> {
    let binding = bindings
        .get(name)
        .ok_or("required authored field binding")?;
    assert_eq!(
        binding.get("state"),
        Some(&json!("present")),
        "original declared field"
    );
    graph.bound(binding.get("field").ok_or("original field address")?)
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
