// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value as Json, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

#[path = "facts_layered_source.rs"]
mod source;

use self::source::{Fact, Site, Sources};
use super::{BARE, OVERRIDDEN, SUB, database};

#[test]
fn layered_contract_facts_keep_each_winning_value_and_level_source() -> TestResult {
    let data = database().build();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&data, input, Vec::new())?;
        let original = Sources::read(&fixture)?;
        let (_, catalog) = fixture.diagnostic()?;
        check_rows(&catalog, &original, input.marker())?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn check_rows(catalog: &Json, original: &Sources, marker: Option<bool>) -> TestResult {
    let bare = catalog_contracts::entry(catalog, &health_origin_fixture::guid(BARE), None)?;
    let parent_guid = health_origin_fixture::guid(OVERRIDDEN);
    let sub_guid = health_origin_fixture::guid(SUB);
    let parent = catalog_contracts::entry(catalog, &parent_guid, None)?;
    let child = catalog_contracts::entry(catalog, &parent_guid, Some(&sub_guid))?;
    let [shared, illegal] = &original.template;
    let [once, max_players, ..] = &original.defaults;
    let [parent_once, parent_illegal, parent_max] = &original.parent;

    for template in &original.template {
        check_fact(
            catalog,
            bare,
            template,
            marker,
            Some(original.bare_template),
            &[],
        )?;
    }
    for default in [once, max_players] {
        check_fact(catalog, bare, default, marker, None, &[])?;
    }
    check_fact(
        catalog,
        parent,
        shared,
        marker,
        Some(original.parent_template),
        &[],
    )?;
    check_fact(
        catalog,
        child,
        &original.sub_share,
        marker,
        None,
        &[shared.payload, original.parent_template],
    )?;
    let shadowed_template = [illegal.payload, original.parent_template];
    for row in [parent, child] {
        for (selected, shadowed) in [
            (parent_once, std::slice::from_ref(&once.payload)),
            (parent_illegal, shadowed_template.as_slice()),
            (parent_max, std::slice::from_ref(&max_players.payload)),
        ] {
            check_fact(catalog, row, selected, marker, None, shadowed)?;
        }
    }
    check_handler_fallbacks(catalog, [bare, parent, child], original, marker)
}

fn check_handler_fallbacks(
    catalog: &Json,
    rows: [&Json; 3],
    original: &Sources,
    marker: Option<bool>,
) -> TestResult {
    let [_, _, availability @ ..] = &original.defaults;
    for row in rows {
        for default in availability {
            check_fact(catalog, row, default, marker, None, &[])?;
        }
        check_fact(
            catalog,
            row,
            &original.handler_hide,
            marker,
            None,
            &[original.shadowed_hide],
        )?;
    }
    Ok(())
}

fn check_fact(
    catalog: &Json,
    row: &Json,
    fact: &Fact,
    marker: Option<bool>,
    dependency: Option<Site>,
    forbidden: &[Site],
) -> TestResult {
    let field = origin::at(row, &format!("/{}", fact.field))?;
    let value = origin::present(field, "value", &fact.value, marker)?;
    let layer = origin::present(field, "layer", &json!(fact.layer), marker)?;
    let value_uses = origin::uses(catalog, value)?;
    let layer_uses = origin::uses(catalog, layer)?;
    assert_use(&value_uses, fact.payload, "payload", fact.field);
    assert!(
        !has_use(&layer_uses, fact.payload, "payload"),
        "{} level does not copy the winning scalar payload",
        fact.field
    );
    for uses in [&value_uses, &layer_uses] {
        assert_use(uses, fact.selector, "selector", fact.field);
        if let Some(dependency) = dependency {
            assert_use(uses, dependency, "selector", fact.field);
        }
        assert!(
            !uses
                .iter()
                .any(|used| forbidden.iter().any(|site| same_site(used, *site))),
            "{} excludes shadowed values and unconsumed dependencies",
            fact.field
        );
    }
    Ok(())
}

fn assert_use(uses: &[&Json], site: Site, role: &str, field: &str) {
    assert!(
        has_use(uses, site, role),
        "{field} retains the original {role} occurrence {site:?}"
    );
}

fn has_use(uses: &[&Json], site: Site, role: &str) -> bool {
    uses.iter().any(|used| {
        same_site(used, site)
            && used.pointer("/location/field").and_then(Json::as_str) == Some("$value")
            && used.get("role").and_then(Json::as_str) == Some(role)
    })
}

fn same_site(used: &Json, site: Site) -> bool {
    used.pointer("/location/namespace").and_then(Json::as_str) == Some("dataforgeValue")
        && used.pointer("/location/indices")
            == Some(&json!([site.record.to_string(), site.ordinal.to_string()]))
}

#[path = "facts_template_failed_read.rs"]
mod template_failed_read;
