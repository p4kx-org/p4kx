// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original objective, handler, payout, modifier, and owner facts.

use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, canonical};
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;
use crate::template_fixture_tags as tags;
use crate::template_fixture_text as texts;

#[path = "catalog_contract_template_flow.rs"]
mod flow;

const OWNER: &str = "80000000-0000-4000-8000-000000000001";
const SCOPE: &str = "80000000-0000-4000-8000-000000000002";

fn handler<'catalog>(graph: &Template<'catalog>, index: usize) -> TestResult<&'catalog Value> {
    graph.field(context::objective(graph, index)?, "objectiveHandler")
}

#[test]
fn objective_texts_are_game_texts_with_the_german_text_missing_in_the_fixture() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let first = context::objective(&graph, 0)?;
    let display = graph.field(first, "displayInfo")?;
    let category = graph.field(display, "category")?;
    assert_eq!(
        fields::text(category)?,
        "Primary",
        "original category spelling"
    );
    assert_eq!(
        fields::vocabulary(&graph, category)?.get("value"),
        Some(&json!("primary")),
        "the contextual category remains recognized"
    );
    fields::scalar_fields(&graph, display, &[("hideOnHUD", "boolean", json!(false))])?;
    texts::localized(
        &catalog,
        &graph,
        graph.field(display, "shortDescription")?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    texts::no_lookup(
        &graph,
        graph.field(display, "objectiveMarkerLabel")?,
        "@LOC_UNINITIALIZED",
        "placeholder",
    )?;
    let overrides = graph.field(first, "overrideMissionDetailsDisplayInfo")?;
    texts::localized(
        &catalog,
        &graph,
        graph.field(overrides, "titleOverride")?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    texts::no_lookup(
        &graph,
        graph.field(overrides, "descriptionOverride")?,
        "@LOC_UNINITIALIZED",
        "placeholder",
    )?;
    Ok(())
}

#[test]
fn a_hauling_handler_states_its_markers_tokens_and_phrasings() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let handler = handler(&graph, 0)?;
    assert_eq!(
        raw::present(
            raw::payload(handler, "struct")?
                .get("sourceType")
                .ok_or("handler type")?
        )?,
        &json!("ObjectiveHandler_Hauling"),
        "the original handler type"
    );
    fields::scalar_fields(&graph, handler, &[("hideMarkers", "boolean", json!(true))])?;
    let settings = graph.field(handler, "objectiveSettings")?;
    assert_eq!(
        fields::text(graph.field(settings, "itemExtendedTextToken")?)?,
        "item",
        "item token"
    );
    assert_eq!(
        fields::text(graph.field(settings, "dropOffLocationExtendedTextToken")?)?,
        "destination",
        "destination token"
    );
    texts::localized(
        &catalog,
        &graph,
        graph.field(graph.field(settings, "andObjective")?, "shortDescription")?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    texts::no_lookup(
        &graph,
        graph.field(
            graph.field(settings, "collectObjective")?,
            "shortDescription",
        )?,
        "@LOC_UNINITIALIZED",
        "placeholder",
    )?;
    fields::absent_field(settings, "orObjective")?;
    fields::absent_field(handler, "location")?;
    fields::absent_field(handler, "completePhaseOnPlayerEnterInstance")?;
    Ok(())
}

#[test]
fn a_local_handler_states_its_radius_travel_text_and_marker_entity() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let handler = handler(&graph, 2)?;
    assert_eq!(
        raw::present(
            raw::payload(handler, "struct")?
                .get("sourceType")
                .ok_or("handler type")?
        )?,
        &json!("ObjectiveHandler_Local"),
        "the original handler type"
    );
    assert_eq!(
        fields::text(graph.field(handler, "module")?)?,
        "Libs/Subsumption/Missions/Goto.xml",
        "the complete original module path"
    );
    fields::numeric_fields(&graph, handler, &[("travelRadiusKM", json!(2.5))])?;
    fields::scalar_fields(
        &graph,
        handler,
        &[("disableTravelObjectives", "boolean", json!(false))],
    )?;
    texts::localized(
        &catalog,
        &graph,
        graph.field(
            graph.field(handler, "travelObjectiveInfo")?,
            "shortDescription",
        )?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    let nav = graph.field(handler, "navPointSpawnInfo")?;
    fields::reference(graph.field(nav, "entityClassDefinition")?, OWNER)?;
    texts::localized(
        &catalog,
        &graph,
        graph.field(nav, "name")?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    assert!(
        raw::elements(graph.field(nav, "extendedTextTokens")?)?.is_empty(),
        "the real empty extended-token array remains empty"
    );
    fields::absent_field(handler, "objectiveSettings")?;
    Ok(())
}

#[test]
fn an_instance_handler_states_its_tags_and_text() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let handler = handler(&graph, 1)?;
    fields::scalar_fields(
        &graph,
        handler,
        &[("completePhaseOnPlayerEnterInstance", "boolean", json!(true))],
    )?;
    fields::numeric_fields(
        &graph,
        handler,
        &[("instanceEnterTravelRadiusKM", json!(4.0))],
    )?;
    texts::localized(
        &catalog,
        &graph,
        graph.field(handler, "instanceHUDObjectiveText")?,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    tags::path(
        &graph,
        graph.field(handler, "instanceTagIdentifier")?,
        Some("Kill"),
    )?;
    let search = fields::elements(&graph, graph.field(handler, "ocTagsToSearchInstanceEnter")?)?;
    assert_eq!(search.len(), 1, "the original ordered tag list");
    tags::path(
        &graph,
        search.first().ok_or("actual search tag")?,
        Some("Kill"),
    )?;
    assert_eq!(
        fields::text(graph.field(handler, "transitToInstanceObjectiveMarker")?)?,
        "Marker",
        "the original objective marker"
    );
    Ok(())
}

#[test]
fn reward_contributions_and_objective_comms_are_exported_as_stored() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let first = context::objective(&graph, 0)?;
    let reward = graph.field(first, "rewardContribution")?;
    assert_eq!(
        raw::present(
            raw::payload(reward, "struct")?
                .get("sourceType")
                .ok_or("reward type")?
        )?,
        &json!("ObjectiveRewardContribution_PercentOfMission"),
        "original reward kind"
    );
    fields::numeric_fields(&graph, reward, &[("percentContribution", json!(60))])?;
    fields::scalar_fields(
        &graph,
        reward,
        &[("doPartialRewardPayout", "boolean", json!(true))],
    )?;
    let comms = fields::elements(&graph, graph.field(first, "commsNotifications")?)?;
    assert_eq!(comms.len(), 1, "all original comms in source order");
    let comm = *comms.first().ok_or("actual original comm")?;
    for (name, original, meaning) in [
        ("trigger", "Completed", "completed"),
        (
            "commsTiming",
            "HUDNotificationFirst",
            "hudNotificationFirst",
        ),
    ] {
        let node = graph.field(comm, name)?;
        assert_eq!(
            fields::text(node)?,
            original,
            "original notification vocabulary"
        );
        assert_eq!(
            fields::vocabulary(&graph, node)?.get("value"),
            Some(&json!(meaning)),
            "the contextual vocabulary remains exact"
        );
    }
    fields::numeric_fields(&graph, comm, &[("delay", json!(2))])?;
    tags::path(
        &graph,
        graph.field(comm, "communicationTags")?,
        Some("Kill"),
    )?;
    let second = context::objective(&graph, 1)?;
    raw::source_null(graph.field(second, "rewardContribution")?);
    assert!(
        raw::elements(graph.field(second, "commsNotifications")?)?.is_empty(),
        "real empty comms"
    );
    Ok(())
}

#[test]
fn the_partial_payout_lists_its_ranges_with_the_scope_links() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let payout = fields::structure(
        &graph,
        fields::required_input(&graph, "partialRewardPayout")?,
    )?;
    let ranges = fields::elements(&graph, graph.field(payout, "percentageThresholds")?)?;
    assert_eq!(ranges.len(), 1, "one actual original payout range");
    let range = *ranges.first().ok_or("actual range")?;
    fields::numeric_fields(
        &graph,
        range,
        &[("minPercentage", json!(76)), ("maxPercentage", json!(99))],
    )?;
    let currency = fields::number(graph.field(range, "currencyRewardMultiplier")?)?
        .as_f64()
        .ok_or("finite original currency multiplier")?;
    assert!((currency - 0.76).abs() < 1e-6, "{currency}");
    let multipliers = fields::elements(&graph, graph.field(range, "reputationMultipliers")?)?;
    assert_eq!(multipliers.len(), 1, "one original scope adjustment");
    let multiplier = *multipliers.first().ok_or("actual scope adjustment")?;
    crate::mission_template_graph::reference(
        graph.field(multiplier, "reputationScope")?,
        SCOPE,
        None,
    )?;
    fields::numeric_fields(
        &graph,
        multiplier,
        &[("reputationRewardMultiplier", json!(2))],
    )?;
    Ok(())
}

#[test]
fn modifiers_are_a_closed_list_tagged_by_their_struct() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let rows = raw::array(graph.projection, "modifiers")?;
    assert_eq!(rows.len(), 4, "all original modifiers in authored order");
    for (row, expected) in rows.iter().zip([
        "securityClearance",
        "requestRentShip",
        "shipKioskFilter",
        "lawLicense",
    ]) {
        assert_eq!(
            raw::present(row.get("kind").ok_or("sourced modifier kind")?)?,
            &json!(expected),
            "the existing closed modifier interpretation"
        );
    }
    crate::template_fixture_modifiers::details(&graph, rows, OWNER)?;
    Ok(())
}

#[test]
fn the_owner_names_its_guid_and_the_class_name_of_its_record() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let owner = graph
        .projection
        .get("owner")
        .filter(|value| !value.is_null())
        .ok_or("actual selected original owner")?;
    let original = graph
        .owner
        .follow(owner.get("source").ok_or("actual owner source")?)?;
    fields::reference(original, OWNER)?;
    assert_eq!(
        raw::present(
            owner
                .get("className")
                .ok_or("actual selected header name")?
        )?,
        &json!("MDT"),
        "the actual selected foreign header class name"
    );
    Ok(())
}
