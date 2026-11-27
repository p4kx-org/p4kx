// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as raw_fields;
use crate::{commodity_v3_fixture, spawns_fixture};

pub(super) fn template_identities(catalog: &Value) -> TestResult {
    let templates = catalog
        .pointer("/definitions/contractTemplates/rows")
        .and_then(Value::as_array)
        .ok_or("canonical physical Template definitions")?;
    let expected = [
        spawns_fixture::TEMPLATE,
        spawns_fixture::ELSEWHERE_TEMPLATE,
        spawns_fixture::DEEP_PHASES,
        spawns_fixture::DEEP_FLOW,
        spawns_fixture::UNKNOWN_FLOW,
        spawns_fixture::UNKNOWN_MODIFIER,
        spawns_fixture::UNKNOWN_VALUE,
        spawns_fixture::UNKNOWN_CONDITION,
        spawns_fixture::UNKNOWN_CONTRACT_PROPERTY,
        spawns_fixture::UNKNOWN_ITEM_CONDITION,
        spawns_fixture::UNKNOWN_LOCATION_CONDITION,
        spawns_fixture::UNKNOWN_LOCATION_AFTER_KNOWN,
    ]
    .map(commodity_v3_fixture::guid);
    assert_eq!(
        templates.len(),
        expected.len(),
        "readable physical sources, including failed required meanings, stay in D; the leftover stays excluded"
    );
    for (row, identity) in templates.iter().zip(expected) {
        assert_eq!(
            raw::present(row.get("guid").ok_or("actual sourced physical identity")?)?,
            &json!(identity),
            "exact retained source identities in canonical GUID order"
        );
    }
    Ok(())
}

pub(super) fn class_settings(graph: &Template<'_>) -> TestResult {
    let class = raw_fields::structure(graph, raw_fields::required_input(graph, "contractClass")?)?;
    assert_eq!(
        raw::payload(class, "struct")?.pointer("/sourceType/value"),
        Some(&json!("ContractClass_Contract")),
        "the exact original concrete class type"
    );
    let settings = raw_fields::structure(graph, graph.field(class, "additionalParams")?)?;
    for (name, expected) in [("canBeShared", true), ("hasCompleteButton", false)] {
        assert_eq!(
            raw_fields::scalar(graph.field(settings, name)?, "boolean")?,
            &json!(expected),
            "the exact original active-setting flag"
        );
    }
    let finish = raw_fields::structure(graph, graph.field(class, "autoFinishSettings")?)?;
    assert_eq!(
        raw_fields::scalar(graph.field(finish, "failIfSentToPrison")?, "boolean")?,
        &json!(true),
        "the exact original auto-finish flag"
    );
    let deadline = raw_fields::structure(graph, graph.field(finish, "contractDeadline")?)?;
    for (name, expected) in [
        ("missionCompletionTime", 300),
        ("remainingTimeToShowTimer", 15),
    ] {
        assert_eq!(
            raw_fields::number(graph.field(deadline, name)?)?,
            &json!(expected),
            "the exact original deadline quantity"
        );
    }
    assert_eq!(
        raw_fields::scalar(graph.field(deadline, "missionAutoEnd")?, "boolean")?,
        &json!(true),
        "the exact original automatic-end flag"
    );
    let state = graph.field(deadline, "missionResultAfterTimerEnd")?;
    assert_eq!(
        raw_fields::text(state)?,
        "Failed",
        "the original stored state label"
    );
    assert_eq!(
        raw_fields::vocabulary(graph, state)?.get("value"),
        Some(&json!("failed")),
        "the unchanged recognized deadline result"
    );
    Ok(())
}

pub(super) fn objective_ancestry(graph: &Template<'_>) -> TestResult {
    let objectives = context::objectives(graph)?;
    let mut ids = Vec::new();
    for objective in objectives {
        let id = raw_fields::scalar(
            graph.bound(objective.get("id").ok_or("actual original id field")?)?,
            "guid",
        )?
        .as_str()
        .ok_or("actual original id")?;
        let parent = match objective.get("parent") {
            Some(Value::Null) => None,
            Some(reference) => Some(
                raw_fields::scalar(graph.field(graph.owner.follow(reference)?, "id")?, "guid")?
                    .as_str()
                    .ok_or("actual original parent id")?,
            ),
            None => return Err("required nullable actual parent link".into()),
        };
        let node = graph
            .owner
            .follow(objective.get("source").ok_or("actual objective source")?)?;
        ids.push((
            id,
            parent,
            raw_fields::text(graph.field(node, "debugName")?)?,
        ));
    }
    assert_eq!(
        ids,
        [
            ("70000000-0000-4000-8000-000000000001", None, "Deliver"),
            (
                "70000000-0000-4000-8000-000000000002",
                Some("70000000-0000-4000-8000-000000000001"),
                ""
            ),
            ("70000000-0000-4000-8000-000000000003", None, "Go"),
        ],
        "the token without an id is left out of the contextual selection"
    );
    Ok(())
}
