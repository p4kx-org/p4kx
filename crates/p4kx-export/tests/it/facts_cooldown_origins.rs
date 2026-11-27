// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Input, TestResult};

#[path = "facts_cooldown_fixture.rs"]
mod fixture;
#[path = "facts_cooldown_source.rs"]
mod source;

use self::fixture::{CHILD, Case, PARENT};
use self::source::{Pick, Site};

#[test]
fn cooldown_scalars_and_layers_keep_the_original_selected_inputs() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let (fixture, original) = fixture::open(Case::Defaults, input)?;
        let (_, catalog) = fixture.diagnostic()?;
        let parent_guid = health_origin_fixture::guid(PARENT);
        let parent = catalog_contracts::entry(&catalog, &parent_guid, None)?;
        check_row(&catalog, parent, &original.parent, input, "handler", None)?;

        let (fixture, original) = fixture::open(Case::Layered, input)?;
        let (_, catalog) = fixture.diagnostic()?;
        let parent_guid = health_origin_fixture::guid(PARENT);
        let child_guid = health_origin_fixture::guid(CHILD);
        let parent = catalog_contracts::entry(&catalog, &parent_guid, None)?;
        let child = catalog_contracts::entry(&catalog, &parent_guid, Some(&child_guid))?;
        check_row(
            &catalog,
            parent,
            &original.parent,
            input,
            "contract",
            original.later,
        )?;
        check_row(
            &catalog,
            child,
            &original.child,
            input,
            "subContract",
            original.later,
        )?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn an_absent_cooldown_and_an_unreadable_cooldown_keep_distinct_source_evidence() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        for case in [Case::Missing, Case::Unreadable] {
            let (fixture, original) = fixture::open(case, input)?;
            let (_, catalog) = fixture.diagnostic()?;
            let guid = health_origin_fixture::guid(PARENT);
            let row = catalog_contracts::entry(&catalog, &guid, None)?;
            let value = origin::at(row, "/hasPersonalCooldown")?;
            let layer = origin::at(row, "/cooldownLayers/hasPersonalCooldown")?;
            let incomplete = original.failed.is_some();
            let state = if incomplete { "unreadable" } else { "absent" };
            for cell in [value, layer] {
                assert_eq!(
                    cell.get("state"),
                    Some(&json!(state)),
                    "the selected field and layer distinguish unreadable from absent"
                );
                assert_eq!(
                    cell.get("value"),
                    Some(&Value::Null),
                    "neither state supplies a selected cooldown value or layer"
                );
                assert_eq!(
                    cell.get("sourceSetComplete"),
                    Some(&json!(!incomplete)),
                    "readable absence can certify the full contributor set"
                );
                if !incomplete {
                    assert_eq!(
                        cell.get("containsEncryptedSource"),
                        Some(&json!(input.marker())),
                        "the absence is based on the actual original input"
                    );
                }
            }
            if let Some(failed) = original.failed {
                failed_attempt(&catalog, failed, &[value, layer])?;
            }
            origin::closure(&catalog)?;
        }
    }
    Ok(())
}

#[test]
fn a_full_contract_override_leaves_the_default_out_of_selection_uses() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let (fixture, original) = fixture::open(Case::FullyOverridden, input)?;
        let (_, catalog) = fixture.diagnostic()?;
        let guid = health_origin_fixture::guid(PARENT);
        let row = catalog_contracts::entry(&catalog, &guid, None)?;
        check_row(&catalog, row, &original.parent, input, "contract", None)?;
        let site = original
            .shadowed_default
            .ok_or("original default occurrence")?;
        check_default_definition(&catalog, row, site)?;
        check_default_selection(&catalog, row, &original.parent, site)?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn check_default_selection(catalog: &Value, row: &Value, picks: &[Pick], site: Site) -> TestResult {
    let prerequisites = origin::cell(row, "requiredIntros", "present")?;
    assert!(
        origin::uses(catalog, prerequisites)?
            .iter()
            .any(|used| value_input(used, site, "selector")),
        "the original availability Struct independently selects handler prerequisites"
    );
    let layers = origin::at(row, "/cooldownLayers")?;
    for pick in picks {
        for owner in [row, layers] {
            let cell = origin::cell(owner, pick.field, "present")?;
            excludes_default(catalog, cell, site)?;
        }
    }
    let summary = origin::cell(row, "cooldownLayer", "present")?;
    excludes_default(catalog, summary, site)
}

fn excludes_default(catalog: &Value, cell: &Value, site: Site) -> TestResult {
    assert!(
        origin::uses(catalog, cell)?
            .iter()
            .all(|used| !same_site(used, site)),
        "a fully overridden cooldown retains no contribution from the original default"
    );
    Ok(())
}

fn check_default_definition(catalog: &Value, row: &Value, site: Site) -> TestResult {
    let input = origin::at(row, "/source/inputRef")?;
    let definitions = origin::rows(catalog, "/definitions/contractGenerators/rows")?;
    let definition = definitions
        .iter()
        .find(|definition| {
            definition.get("inputRef") == Some(input)
                && definition
                    .pointer("/recordIndex/value")
                    .and_then(Value::as_u64)
                    == Some(u64::from(site.record))
        })
        .ok_or("original physical Generator definition")?;
    let original = origin::rows(definition, "/source/values")?
        .get(usize::try_from(site.ordinal)?)
        .ok_or("original default Struct in the flat arena")?;
    let cell = origin::cell(original, "value", "present")?;
    assert_eq!(
        origin::at(cell, "/value/sourceType/value")?,
        "CooldownAvailability",
        "the selected original occurrence is the default availability Struct"
    );
    assert!(
        origin::uses(catalog, cell)?
            .iter()
            .any(|used| value_input(used, site, "payload")),
        "the unchanged definition retains the original default container"
    );
    Ok(())
}

fn check_row(
    catalog: &Value,
    row: &Value,
    picks: &[Pick],
    input: Input,
    nearest: &str,
    later: Option<Site>,
) -> TestResult {
    let layers = origin::at(row, "/cooldownLayers")?;
    let summary = origin::present(row, "cooldownLayer", &json!(nearest), input.marker())?;
    let aggregate = origin::uses(catalog, summary)?;
    assert_eq!(
        picks.len(),
        5,
        "one selected original input for each cooldown field"
    );
    for pick in picks {
        let cell = origin::present(row, pick.field, &pick.value, input.marker())?;
        let chosen_layer = origin::present(layers, pick.field, &json!(pick.layer), input.marker())?;
        let value_uses = origin::uses(catalog, cell)?;
        let decision_uses = origin::uses(catalog, chosen_layer)?;
        assert!(
            value_uses
                .iter()
                .any(|used| value_input(used, pick.datum, pick.role)),
            "{} retains its actual selected original primitive and role",
            pick.field
        );
        if let Some(parameter) = pick.parameter {
            assert!(
                value_uses
                    .iter()
                    .any(|used| value_input(used, parameter, "selector")),
                "{} retains the selected authored parameter name",
                pick.field
            );
        }
        check_choice(&decision_uses, pick);
        check_choice(&aggregate, pick);
        for used in value_uses.iter().chain(&decision_uses).chain(&aggregate) {
            let physical = origin::source(catalog, used)?;
            assert_eq!(
                physical.get("kind"),
                Some(&json!(source_kind(input))),
                "cooldown facts are bound to the actual Generator input kind"
            );
            assert!(
                later.is_none_or(|later| !same_site(used, later)),
                "a later case-equivalent override does not enter any selected contribution set"
            );
        }
    }
    Ok(())
}

fn check_choice(uses: &[&Value], pick: &Pick) {
    let chosen = pick.parameter.unwrap_or(pick.datum);
    assert!(
        uses.iter().any(|used| {
            same_site(used, chosen) && used.get("role").and_then(Value::as_str) == Some("selector")
        }),
        "{} retains the original input that selects its level",
        pick.field
    );
}

const fn source_kind(input: Input) -> &'static str {
    match input {
        Input::Plain | Input::Encrypted => "archiveEntry",
        Input::Standalone => "file",
    }
}

fn failed_attempt(catalog: &Value, failed: Site, cells: &[&Value]) -> TestResult {
    let inventory = origin::rows(catalog, "/provenance/uses")?;
    assert!(
        inventory
            .iter()
            .any(|used| value_input(used, failed, "attempted")),
        "the rejected original Bool byte remains a failed read in the inventory"
    );
    for cell in cells {
        assert!(
            !origin::uses(catalog, cell)?
                .iter()
                .any(|used| same_site(used, failed)),
            "an unreadable primitive does not enter the contribution set"
        );
    }
    Ok(())
}

fn value_input(used: &Value, site: Site, role: &str) -> bool {
    same_site(used, site)
        && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
        && used.get("role").and_then(Value::as_str) == Some(role)
}

fn same_site(used: &Value, site: Site) -> bool {
    used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeValue")
        && used.pointer("/location/indices")
            == Some(&json!([site.record.to_string(), site.ordinal.to_string()]))
}
