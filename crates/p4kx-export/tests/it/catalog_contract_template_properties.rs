// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original objective-property source cells and contextual bindings.

use std::error::Error;

use serde_json::{Value, json};

use crate::catalog_contract_templates::{build, canonical};
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;
use crate::template_fixture_tags as tags;
use crate::template_fixture_text as texts;

#[path = "catalog_contract_template_property_diagnostics.rs"]
mod diagnostics;

type TestResult = Result<(), Box<dyn Error>>;

const OBJECTIVE_A: &str = "70000000-0000-4000-8000-000000000001";
const OWNER: &str = "80000000-0000-4000-8000-000000000001";

#[test]
fn a_referenced_property_names_the_property_of_the_template_it_points_to() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let properties = context::properties(&graph, Some(0))?;
    let referenced = *properties.first().ok_or("actual referenced property")?;
    assert_eq!(
        referenced.get("kind"),
        Some(&json!("referenced")),
        "actual objective-property kind"
    );
    assert_eq!(
        context::binding_text(&graph, referenced, "variable")?,
        Some("Cargo_BP"),
        "caller variable"
    );
    assert_eq!(
        context::binding_text(&graph, referenced, "token")?,
        Some("Cargo"),
        "caller token"
    );
    let caller = graph.owner.follow(
        referenced
            .get("contractProperty")
            .ok_or("actual property weak slot")?,
    )?;
    context::property_target(&graph, caller, "Place_BP")?;
    assert_eq!(
        referenced.get("value"),
        Some(&Value::Null),
        "no invented owned value for a reference"
    );
    assert_eq!(
        referenced.get("linkedOutput"),
        Some(&Value::Null),
        "no invented output link"
    );
    Ok(())
}

#[test]
fn output_and_embedded_properties_state_their_values_by_type() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let properties = context::properties(&graph, Some(0))?;
    assert_eq!(
        properties.len(),
        11,
        "all original objective properties in order"
    );
    let output = *properties.get(1).ok_or("actual output property")?;
    assert_eq!(
        output.get("kind"),
        Some(&json!("output")),
        "output naming kind"
    );
    assert_eq!(
        context::binding_text(&graph, output, "variable")?,
        Some("Out_BP"),
        "original output name"
    );
    context::kind(&graph, output, "object")?;
    let flag = *properties.get(2).ok_or("actual boolean property")?;
    context::kind(&graph, flag, "boolean")?;
    fields::scalar_fields(
        &graph,
        context::value(&graph, flag)?,
        &[("value", "boolean", json!(true))],
    )?;
    let string = *properties.get(3).ok_or("actual StringHash property")?;
    context::kind(&graph, string, "stringHash")?;
    crate::template_fixture_options::string(&catalog, &graph, context::value(&graph, string)?)?;
    let integer = *properties.get(4).ok_or("actual integer property")?;
    assert_integer_property(&graph, integer)?;
    let tag_property = *properties.get(5).ok_or("actual tag property")?;
    context::kind(&graph, tag_property, "tags")?;
    crate::template_fixture_options::tags(&graph, context::value(&graph, tag_property)?)?;
    Ok(())
}

fn assert_integer_property(graph: &Template<'_>, integer: &Value) -> TestResult {
    context::kind(graph, integer, "integer")?;
    let options = fields::elements(
        graph,
        graph.field(context::value(graph, integer)?, "options")?,
    )?;
    assert_eq!(options.len(), 1, "one original integer option");
    let option = *options.first().ok_or("actual integer option")?;
    texts::no_lookup(
        graph,
        graph.field(option, "textId")?,
        "@LOC_UNINITIALIZED",
        "placeholder",
    )?;
    fields::numeric_fields(
        graph,
        option,
        &[
            ("value", json!(5)),
            ("variation", json!(1)),
            ("weighting", json!(2.0)),
        ],
    )?;
    fields::scalar_fields(
        graph,
        option,
        &[("DEBUG_forceChooseThisOption", "boolean", json!(false))],
    )?;
    assert!(
        raw::elements(graph.field(option, "dependentProperties")?)?.is_empty(),
        "original dependencies empty"
    );
    Ok(())
}

#[test]
fn npc_entity_and_organization_values_link_or_state_their_fields() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let properties = context::properties(&graph, Some(0))?;
    context::spawn(
        &catalog,
        &graph,
        properties.get(6).ok_or("NPC property")?,
        "npcSpawnDescriptions",
        "contractNpcSpawns",
    )?;
    context::spawn(
        &catalog,
        &graph,
        properties.get(9).ok_or("entity property")?,
        "entitySpawnDescriptions",
        "contractEntitySpawns",
    )?;
    let property = *properties.get(10).ok_or("organization property")?;
    context::kind(&graph, property, "organization")?;
    let value = context::value(&graph, property)?;
    fields::reference(graph.field(value, "debugForceChosenOrganization")?, OWNER)?;
    let conditions = fields::elements(&graph, graph.field(value, "matchConditions")?)?;
    assert_eq!(conditions.len(), 1, "one original organization condition");
    let condition = *conditions.first().ok_or("actual organization condition")?;
    let meaning = context::context(&graph, "valueMeanings", "source", value)?;
    let selected = raw::array(meaning, "conditions")?;
    assert_eq!(
        selected.len(),
        1,
        "one recognized condition for this original occurrence"
    );
    assert_eq!(
        raw::present(
            selected
                .first()
                .and_then(|row| row.get("kind"))
                .ok_or("condition kind")?
        )?,
        &json!("specificOrganizations"),
        "the exact condition interpretation"
    );
    let organizations = fields::elements(&graph, graph.field(condition, "organizations")?)?;
    assert_eq!(organizations.len(), 1, "the original organization list");
    fields::reference(organizations.first().ok_or("original organization")?, OWNER)?;
    Ok(())
}

#[test]
fn a_ship_spawn_or_location_value_links_to_the_shared_kinds() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let properties = context::properties(&graph, Some(0))?;
    context::spawn(
        &catalog,
        &graph,
        properties.get(7).ok_or("ship property")?,
        "shipSpawnDescriptions",
        "contractShipSpawns",
    )?;
    context::spawn(
        &catalog,
        &graph,
        properties.get(8).ok_or("location property")?,
        "location",
        "contractLocationQueries",
    )?;
    Ok(())
}

#[test]
fn an_input_property_names_the_output_and_the_objective_it_takes() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let properties = context::properties(&graph, Some(1))?;
    assert_eq!(properties.len(), 1, "the original phase input");
    let input = *properties.first().ok_or("actual input property")?;
    assert_eq!(
        input.get("kind"),
        Some(&json!("input")),
        "the recognized objective-property kind"
    );
    assert_eq!(
        context::binding_text(&graph, input, "variable")?,
        Some("In_BP"),
        "actual input name"
    );
    context::kind(&graph, input, "deliveryOrder")?;
    let caller = graph
        .owner
        .follow(input.get("linkedOutput").ok_or("actual output weak slot")?)?;
    let row = context::context(&graph, "linkedOutputs", "caller", caller)?;
    assert_eq!(
        row.get("selected"),
        Some(&json!(true)),
        "the actual output was selected"
    );
    let variable = row
        .get("variable")
        .ok_or("actual selected output variable")?;
    assert_eq!(
        variable.get("interpretation"),
        Some(&json!("value")),
        "usable output variable"
    );
    assert_eq!(
        fields::text(graph.bound(variable.get("field").ok_or("actual output field")?)?)?,
        "Out_BP",
        "the selected original output name"
    );
    let objective = row.get("objective").ok_or("actual output objective")?;
    assert_eq!(
        objective.get("interpretation"),
        Some(&json!("id")),
        "usable output objective id"
    );
    assert_eq!(
        fields::scalar(
            graph.bound(objective.get("field").ok_or("actual objective id field")?)?,
            "guid"
        )?,
        &json!(OBJECTIVE_A),
        "the actual enclosing objective identity"
    );
    Ok(())
}

#[test]
fn a_nil_tag_is_no_tag_path_in_a_list_or_in_a_single_tag_field() -> TestResult {
    let catalog = build()?;
    let templates = raw::array(&catalog, "missionLocationTemplates")?;
    assert_eq!(
        templates.first().and_then(|row| row.get("tags")),
        Some(&json!(["Missions.Hijacked"])),
        "the nil tag in the list is left out"
    );
    let graph = canonical(&catalog)?;
    let phase = graph.field(context::objective(&graph, 2)?, "missionPhaseIdentifierTag")?;
    tags::path(&graph, phase, None)?;
    Ok(())
}
