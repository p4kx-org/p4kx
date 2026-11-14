// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Contract templates in the Basetool catalog.

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::loot_v3_graph as raw;
use crate::spawns_fixture;
use crate::template_canonical_graph::Template;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as raw_fields;

type TestResult = Result<(), Box<dyn Error>>;

#[path = "catalog_contract_template_checks.rs"]
mod checks;

const TEMPLATE_GUID: &str = "40000000-0000-4000-8000-000000000001";

pub(crate) fn build() -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        spawns_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000));
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

pub(crate) fn canonical(catalog: &Value) -> Result<Template<'_>, Box<dyn Error>> {
    Template::new(catalog, TEMPLATE_GUID)
}

#[test]
fn a_contract_template_guid_resolves_to_an_entry_and_a_leftover_is_left_out() -> TestResult {
    let catalog = build()?;
    assert!(
        catalog.get("contractTemplates").is_none(),
        "no retired root mirror"
    );
    checks::template_identities(&catalog)?;
    for contract in crate::catalog_contracts::v3::rows(&catalog)? {
        assert_eq!(
            contract["templateGuid"], TEMPLATE_GUID,
            "the same original contract link"
        );
    }
    let graph = canonical(&catalog)?;
    assert_eq!(
        raw::present(
            graph
                .definition
                .get("className")
                .ok_or("actual class header")?
        )?,
        &json!("Hunt"),
        "the original header-name conversion"
    );
    assert_eq!(
        raw::present(
            graph
                .definition
                .get("recordPath")
                .ok_or("actual path header")?
        )?,
        &json!("libs/foundry/records/contracts/contracttemplates/hunt"),
        "the original path conversion"
    );
    Ok(())
}

#[test]
fn display_info_class_settings_and_deadline_are_exported_as_stored() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let title = graph.owner.follow(
        graph
            .projection
            .get("title")
            .ok_or("actual title source link")?,
    )?;
    crate::template_fixture_text::localized(
        &catalog,
        &graph,
        title,
        "@place_name",
        Some("Hijacked Ship"),
        None,
    )?;
    let display =
        raw_fields::structure(&graph, raw_fields::required_input(&graph, "displayInfo")?)?;
    crate::mission_template_graph::reference(
        graph.field(display, "type")?,
        "50000000-0000-4000-8000-000000000001",
        None,
    )?;
    for (name, expected) in [
        ("illegal", true),
        ("showLifeTimeInMobiGlas", false),
        ("preShowObjectives", true),
    ] {
        assert_eq!(
            raw_fields::scalar(graph.field(display, name)?, "boolean")?,
            &json!(expected),
            "the exact original display flag"
        );
    }
    assert_eq!(
        raw_fields::scalar(
            raw_fields::required_input(&graph, "notForRelease")?,
            "boolean"
        )?,
        &json!(false),
        "the exact original release flag"
    );
    assert!(
        raw_fields::input(&graph, "workInProgress")?.is_none(),
        "the original absent declaration remains absence, never an invented null value"
    );
    checks::class_settings(&graph)
}

#[test]
fn properties_list_the_variables_with_the_kind_of_their_value() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let mut properties = Vec::new();
    for property in context::properties(&graph, None)? {
        let variable = context::binding_text(&graph, property, "variable")?
            .ok_or("actual original variable")?;
        let token = context::binding_text(&graph, property, "token")?
            .ok_or("actual original stored token, including empty text")?;
        let value = graph.owner.follow(
            property
                .get("value")
                .ok_or("actual selected value source")?,
        )?;
        let source_type = raw::payload(value, "struct")?
            .get("sourceType")
            .and_then(|cell| cell.get("value"))
            .and_then(Value::as_str)
            .ok_or("actual original value type")?;
        let suffix = source_type
            .strip_prefix("MissionPropertyValue_")
            .ok_or("original property family")?;
        properties.push((variable, token, suffix));
    }
    assert_eq!(
        properties,
        [
            ("Wave_BP", "", "ShipSpawnDescriptions"),
            ("Place_BP", "Place", "Location"),
            ("Home_BP", "", "Location"),
            ("Flag_BP", "", "Boolean"),
            ("Cargo_BP", "Cargo", "HaulingOrders"),
            ("Guard_BP", "", "NPCSpawnDescriptions"),
            ("Spare_BP", "", "NPCSpawnDescriptions"),
            ("Loot_BP", "", "EntitySpawnDescriptions"),
            ("Item_BP", "", "MissionItem"),
            ("Name_BP", "", "AIName"),
            ("Combined_BP", "", "CombinedDataSetEntries"),
            ("Pay_BP", "", "Reward"),
            ("Race_BP", "", "TimeTrialRace"),
            ("Map_BP", "", "StarMapLocation"),
        ],
        "every original variable/token/type and its order remain"
    );
    Ok(())
}

#[test]
fn ship_spawns_and_location_queries_link_to_the_shared_kinds_at_the_template_level() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let ships = context::pools(&graph, "ships")?;
    assert_eq!(
        ships
            .iter()
            .map(|row| (row.variable, row.level))
            .collect::<Vec<_>>(),
        [("Wave_BP", "template")],
        "the same selected ship source and level"
    );
    let locations = context::pools(&graph, "location")?;
    assert_eq!(
        locations
            .iter()
            .map(|row| (row.variable, row.level))
            .collect::<Vec<_>>(),
        [("Home_BP", "template"), ("Place_BP", "template")],
        "the same source ordering and level"
    );
    let pool = ships
        .first()
        .ok_or("actual registered ship pool")?
        .identifier;
    assert!(
        catalog["contractShipSpawns"]
            .as_array()
            .ok_or("pools")?
            .iter()
            .any(|spawn| spawn.get("guid") == Some(pool)),
        "the link resolves"
    );
    Ok(())
}

#[test]
fn hauling_orders_name_the_property_and_the_objective_source() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    let orders = raw::array(graph.projection, "haulingOrders")?;
    let mut sources = Vec::new();
    for order in orders {
        let variable = match order.get("variable") {
            Some(Value::Null) => None,
            Some(reference) => Some(raw_fields::text(graph.bound(reference)?)?),
            None => return Err("required nullable actual variable link".into()),
        };
        sources.push((
            order
                .get("source")
                .and_then(Value::as_str)
                .ok_or("actual hauling source level")?,
            variable,
        ));
    }
    assert_eq!(
        sources,
        [("template", Some("Cargo_BP")), ("templateObjective", None)],
        "the same source levels and original variable"
    );
    let item = orders
        .get(1)
        .and_then(|row| row.pointer("/requirements/0/options/0/0/source"))
        .ok_or("the actual grouped objective hauling item")?;
    let item = graph.owner.follow(item)?;
    assert_eq!(
        raw_fields::number(graph.field(item, "maxSCU")?)?,
        &json!(8),
        "the same actual grouped resource quantity"
    );
    Ok(())
}

#[test]
fn objectives_follow_the_phases_depth_first_and_skip_a_token_without_an_id() -> TestResult {
    let catalog = build()?;
    let graph = canonical(&catalog)?;
    checks::objective_ancestry(&graph)?;
    let first = context::objective(&graph, 0)?;
    let second = context::objective(&graph, 1)?;
    assert_eq!(
        raw_fields::scalar(graph.field(first, "startsActive")?, "boolean")?,
        &json!(true),
        "original active flag"
    );
    assert_eq!(
        raw_fields::scalar(graph.field(second, "startsActive")?, "boolean")?,
        &json!(false),
        "original inactive flag"
    );
    let tag = graph.field(second, "missionPhaseIdentifierTag")?;
    crate::template_fixture_tags::path(&graph, tag, Some("Kill"))?;
    crate::template_fixture_tags::path(
        &graph,
        graph.field(first, "missionPhaseIdentifierTag")?,
        None,
    )?;
    assert_eq!(
        raw_fields::text(graph.field(second, "variableName")?)?,
        "PhaseB",
        "original phase variable"
    );
    assert_eq!(
        raw_fields::number(graph.field(second, "weight")?)?,
        &json!(0.5),
        "original phase weight"
    );
    let empty = graph.field(first, "variableName")?;
    assert_eq!(
        raw_fields::text(empty)?,
        "",
        "the actual empty original variable remains present"
    );
    assert_eq!(
        empty.pointer("/value/state"),
        Some(&json!("present")),
        "the old nullable no-variable projection never replaces the actual empty source datum"
    );
    Ok(())
}
