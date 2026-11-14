// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The objective handlers and texts, payouts, modifiers, owner, mission flow and comms of the
//! contract templates in the Basetool catalog of a real Archive.

use std::collections::HashSet;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CraftingWarning, Kind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

#[path = "real_archive_template_parts_flow.rs"]
mod flow;

#[path = "real_archive_template_parts_objectives.rs"]
mod objectives;

use self::flow::Flows;
use self::objectives::English;
use crate::catalog_v3_consumer as reader;
use crate::real_archive_contract_templates::mission_graph::{self as graph, Family, Mission};
use crate::real_archive_verification::{TestResult, guids, print_line};

type PartsResult<T = ()> = Result<T, Box<dyn Error>>;

/// What the parts of the templates hold.
#[derive(Default)]
struct Figures {
    flow_triggers: usize,
    flow_objective_links: usize,
    unresolved_objective_links: usize,
    handlers: usize,
    handler_property_links: usize,
    resolved_texts: usize,
    payout_ranges: usize,
    modifiers: usize,
    comms: usize,
    properties: usize,
    property_values: usize,
    referenced_links: usize,
}

fn check_template(
    mission: &Mission<'_>,
    (scopes, items): (&HashSet<&str>, &HashSet<&str>),
    flows: &Flows<'_>,
    english: &English<'_>,
    figures: &mut Figures,
) -> PartsResult {
    let name = graph::text(mission.definition, "/className/value")?;
    flows.count(mission, name, figures)?;
    objectives::count(mission, name, items, english, figures)?;
    check_payouts(mission, name, scopes, figures)?;
    for modifier in graph::array(mission.projection, "modifiers")? {
        mission.follow(required(modifier, "source")?)?;
        figures.modifiers += 1;
    }
    figures.comms += usize::from(input_selection(mission, "startComms")?.is_some())
        + structs(mission, input_declaration(mission, "endComms")?)?.len();
    let owner = required(mission.projection, "owner")?;
    let owner_name = owner
        .get("className")
        .map(reader::source_value)
        .transpose()?
        .flatten();
    assert!(
        owner_name.is_some_and(Value::is_string),
        "{name}: the owner names its Record"
    );
    Ok(())
}

fn check_payouts(
    mission: &Mission<'_>,
    name: &str,
    scopes: &HashSet<&str>,
    figures: &mut Figures,
) -> PartsResult {
    let Some(payout) = input_selection(mission, "partialRewardPayout")? else {
        return Ok(());
    };
    for range in structs(mission, mission.member(payout, "percentageThresholds")?)? {
        figures.payout_ranges += 1;
        for adjustment in structs(mission, mission.member(range, "reputationMultipliers")?)? {
            let guid =
                linked(mission.member(adjustment, "reputationScope")?)?.ok_or("scopeGuid")?;
            assert!(scopes.contains(guid), "{name}: payout scope {guid}");
        }
    }
    Ok(())
}

fn input_selection<'catalog>(
    mission: &Mission<'catalog>,
    name: &str,
) -> PartsResult<Option<&'catalog Value>> {
    let input = required(required(mission.projection, "inputs")?, name)?;
    nullable(input, "selectedStruct")?
        .map(|reference| mission.follow(reference))
        .transpose()
        .map_err(Into::into)
}

fn input_declaration<'catalog>(
    mission: &Mission<'catalog>,
    name: &str,
) -> PartsResult<Option<&'catalog Value>> {
    let input = required(required(mission.projection, "inputs")?, name)?;
    let declaration = required(input, "declaration")?;
    Ok(match graph::text(declaration, "/state")? {
        "present" => Some(mission.field(required(declaration, "field")?)?),
        "absent" => None,
        _ => return Err("the source input declaration state is invalid".into()),
    })
}

fn required<'value>(value: &'value Value, name: &str) -> PartsResult<&'value Value> {
    value
        .get(name)
        .ok_or_else(|| format!("required canonical field {name}").into())
}

fn nullable<'value>(value: &'value Value, name: &str) -> PartsResult<Option<&'value Value>> {
    let value = required(value, name)?;
    Ok((!value.is_null()).then_some(value))
}

fn selected<'catalog>(
    mission: &Mission<'catalog>,
    parent: &'catalog Value,
    name: &str,
) -> PartsResult<Option<&'catalog Value>> {
    let Some(value) = mission.member(parent, name)? else {
        return Ok(None);
    };
    match node_type(value) {
        Some("struct" | "array") => Ok(mission.first_struct(value)?),
        _ => Ok(None),
    }
}

fn structs<'catalog>(
    mission: &Mission<'catalog>,
    source: Option<&'catalog Value>,
) -> PartsResult<Vec<&'catalog Value>> {
    let Some(source) = source.filter(|node| node_type(node) == Some("array")) else {
        return Ok(Vec::new());
    };
    Ok(mission
        .elements(source)?
        .into_iter()
        .filter(|node| node_type(node) == Some("struct"))
        .collect())
}

fn linked(source: Option<&Value>) -> PartsResult<Option<&str>> {
    let Some(source) = source else {
        return Ok(None);
    };
    if source
        .pointer("/originalError/type")
        .and_then(Value::as_str)
        == Some("danglingReference")
        || node_type(source) == Some("recordReference")
    {
        return Ok(graph::reference(source)?);
    }
    if node_type(source) == Some("weakReference")
        && source
            .pointer("/value/value/value/target/type")
            .and_then(Value::as_str)
            == Some("record")
    {
        return Ok(Some(graph::text(
            source,
            "/value/value/value/recordGuid/value",
        )?));
    }
    Ok(None)
}

fn node_type(value: &Value) -> Option<&str> {
    value.pointer("/value/value/type").and_then(Value::as_str)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_parts_of_the_contract_templates_of_a_real_archive_resolve() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let unknown: Vec<&str> = built
        .summary()
        .warnings()
        .iter()
        .filter(|warning| warning.kind() == "unknown_template_values")
        .flat_map(CraftingWarning::details)
        .map(String::as_str)
        .collect();
    assert!(
        unknown.is_empty(),
        "stored names the catalog does not know: {unknown:?}"
    );
    let scopes = guids(&catalog, "reputationScopes")?;
    let items = guids(&catalog, "items")?;
    let flows = Flows::read(&catalog)?;
    let english = English::read(&catalog)?;
    let templates = graph::contexts(&catalog, Family::Template)?;
    assert_eq!(
        templates.len(),
        reader::rows(&catalog, Kind::ContractTemplates)?.len(),
        "every available original Template has its physical configuration"
    );
    let mut figures = Figures::default();
    for template in &templates {
        check_template(template, (&scopes, &items), &flows, &english, &mut figures)?;
    }
    assert_template_figures(&figures);
    Ok(())
}

fn assert_template_figures(figures: &Figures) {
    print_line(&format!(
        "{} flow triggers ({} objective links, {} unresolved), {} handlers ({} property links), {} resolved texts, {} payout ranges, {} modifiers, {} comms, {} objective properties ({} with a value, {} referenced)",
        figures.flow_triggers,
        figures.flow_objective_links,
        figures.unresolved_objective_links,
        figures.handlers,
        figures.handler_property_links,
        figures.resolved_texts,
        figures.payout_ranges,
        figures.modifiers,
        figures.comms,
        figures.properties,
        figures.property_values,
        figures.referenced_links,
    ));
    assert!(figures.flow_triggers > 500, "flow triggers");
    assert!(figures.flow_objective_links > 500, "flow links");
    assert!(
        figures.unresolved_objective_links <= 5,
        "a flow link that points to no objective"
    );
    assert!(figures.handlers > 500, "handlers");
    assert!(
        figures.handler_property_links > 100,
        "handler property links"
    );
    assert!(figures.resolved_texts > 100, "texts");
    assert!(figures.payout_ranges > 100, "payout ranges");
    assert!(figures.modifiers > 50, "modifiers");
    assert!(figures.comms > 100, "comms");
    assert!(figures.properties > 5_000, "objective properties");
    assert!(figures.property_values > 300, "objective property values");
    assert!(
        figures.referenced_links > 4_500,
        "referenced properties that name a property"
    );
}
