// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The objective handlers and texts, payouts, modifiers, owner, mission flow and comms of the
//! contract templates in the Basetool catalog of a real Archive.

use std::collections::HashSet;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

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
}

fn guids<'c>(catalog: &'c Value, kind: &str) -> Result<HashSet<&'c str>, String> {
    Ok(catalog[kind]
        .as_array()
        .ok_or_else(|| format!("{kind} is a list"))?
        .iter()
        .filter_map(|entry| entry["guid"].as_str())
        .collect())
}

/// Collects the objective ids a condition names, `None` for a link that points to nothing.
fn condition_links<'c>(condition: &'c Value, found: &mut Vec<Option<&'c str>>) {
    match condition["type"].as_str() {
        Some("tokenState") => found.push(condition["objectiveId"].as_str()),
        Some("and" | "or") => {
            for inner in condition["conditions"].as_array().into_iter().flatten() {
                condition_links(inner, found);
            }
        }
        Some("not") => condition_links(&condition["condition"], found),
        _ => {}
    }
}

fn texts_in(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            usize::from(map.get("en").is_some_and(Value::is_string))
                + map.values().map(texts_in).sum::<usize>()
        }
        Value::Array(items) => items.iter().map(texts_in).sum(),
        _ => 0,
    }
}

/// Counts the links of the mission flow of `template` and asserts that every one names an
/// objective among `ids`.
fn check_flow(name: &str, template: &Value, ids: &HashSet<&str>, figures: &mut Figures) {
    for trigger in template["missionFlow"].as_array().into_iter().flatten() {
        figures.flow_triggers += 1;
        let mut links = Vec::new();
        condition_links(&trigger["condition"], &mut links);
        let action = &trigger["action"];
        if action["type"] == "tokenStateChange" {
            links.push(action["objectiveId"].as_str());
        }
        for phase in action["phases"].as_array().into_iter().flatten() {
            links.push(phase["objectiveId"].as_str());
        }
        for link in links {
            match link {
                Some(id) => {
                    figures.flow_objective_links += 1;
                    assert!(ids.contains(id), "{name}: flow names objective {id}");
                }
                None => figures.unresolved_objective_links += 1,
            }
        }
    }
}

/// Counts the texts, handlers and comms of the objectives of `template` and asserts that every
/// marker entity is an entry of `items`.
fn check_objectives(
    name: &str,
    objectives: &[Value],
    items: &HashSet<&str>,
    figures: &mut Figures,
) {
    for objective in objectives {
        figures.comms += objective["comms"].as_array().map_or(0, Vec::len);
        figures.resolved_texts += texts_in(&objective["display"]) + texts_in(&objective["handler"]);
        let handler = &objective["handler"];
        if handler.is_null() {
            continue;
        }
        figures.handlers += 1;
        for field in [
            "locationProperty",
            "organizationProperty",
            "haulingLimitProperty",
            "targetProperty",
        ] {
            figures.handler_property_links += usize::from(handler[field].is_string());
        }
        let marker = handler.pointer("/navPoint/entityClassGuid");
        if let Some(guid) = marker.and_then(Value::as_str) {
            assert!(items.contains(guid), "{name}: marker entity {guid}");
        }
    }
}

/// Asserts that every link the parts of a template state names an entry of its kind or an
/// objective of the same template, and counts what the parts hold.
fn check_template(
    template: &Value,
    kinds: (&HashSet<&str>, &HashSet<&str>),
    figures: &mut Figures,
) -> TestResult {
    let (scopes, items) = kinds;
    let name = template["className"].as_str().unwrap_or_default();
    let objectives = template["objectives"].as_array().ok_or("objectives")?;
    let ids: HashSet<&str> = objectives
        .iter()
        .filter_map(|objective| objective["id"].as_str())
        .collect();
    check_flow(name, template, &ids, figures);
    check_objectives(name, objectives, items, figures);
    let thresholds = template.pointer("/partialPayout/thresholds");
    for range in thresholds.and_then(Value::as_array).into_iter().flatten() {
        figures.payout_ranges += 1;
        for adjustment in range["reputationMultipliers"]
            .as_array()
            .ok_or("multipliers")?
        {
            let guid = adjustment["scopeGuid"].as_str().ok_or("scopeGuid")?;
            assert!(scopes.contains(guid), "{name}: payout scope {guid}");
        }
    }
    figures.modifiers += template["modifiers"].as_array().map_or(0, Vec::len);
    figures.comms += usize::from(!template["startComms"].is_null())
        + template["endComms"].as_array().map_or(0, Vec::len);
    let owner_name = template.pointer("/owner/className");
    assert!(
        owner_name.is_some_and(Value::is_string),
        "{name}: the owner names its Record"
    );
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "the figures are the result of the verification"
)]
fn print_line(line: &str) {
    println!("{line}");
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_parts_of_the_contract_templates_of_a_real_archive_resolve() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let scopes = guids(&catalog, "reputationScopes")?;
    let items = guids(&catalog, "items")?;
    let mut figures = Figures::default();
    for template in catalog["contractTemplates"].as_array().ok_or("templates")? {
        check_template(template, (&scopes, &items), &mut figures)?;
    }
    print_line(&format!(
        "{} flow triggers ({} objective links, {} unresolved), {} handlers ({} property links), {} resolved texts, {} payout ranges, {} modifiers, {} comms",
        figures.flow_triggers,
        figures.flow_objective_links,
        figures.unresolved_objective_links,
        figures.handlers,
        figures.handler_property_links,
        figures.resolved_texts,
        figures.payout_ranges,
        figures.modifiers,
        figures.comms,
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
    Ok(())
}
