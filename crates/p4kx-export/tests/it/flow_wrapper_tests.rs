// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogOptions, ExportDiagnosticCode, build_catalog_from};
use p4kx_extraction::CancelToken;
use serde_json::{Value, json};

use crate::broker_flow_graph::Broker;
use crate::health_origin_fixture::{self as source, Fixture, Input, TestResult};
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;

#[path = "flow_wrapper_declared_target_tests.rs"]
mod declared_target;
#[path = "flow_wrapper_fixture.rs"]
mod fixture;
#[path = "flow_wrapper_wire.rs"]
mod wire;

use self::fixture::Case;

#[test]
fn flow_wrappers_export_all_slots_weights_and_local_links_for_both_owner_families() -> TestResult {
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = fixture::open(Case::Mixed, kind, true)?;
        let built = build_catalog_from(&fixture.archive, &fixture.entry, &source::options())?;
        assert!(
            built.summary().export_health().is_complete(),
            "readable new action kinds publish"
        );
        let catalog: Value = serde_json::from_slice(built.json())?;
        let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
        assert!(
            schema.is_valid(&catalog),
            "new wrapper and entry schemas admit writer output"
        );
        let template = Template::new(&catalog, &source::guid(fixture::TEMPLATE))?;
        let broker = Broker::for_guid(&catalog, &source::guid(fixture::BROKER))?;
        broker.projected(&catalog)?;
        for (owner, triggers, definition, objective) in [
            (
                &template.owner,
                template.triggers()?,
                template.definition,
                fixture::TEMPLATE_OBJECTIVE,
            ),
            (
                &broker.owner,
                raw::array(broker.configuration, "triggers")?,
                broker.definition,
                fixture::BROKER_OBJECTIVE,
            ),
        ] {
            wire::check_mixed(
                owner,
                wire::root(owner, triggers)?,
                &wire::Evidence {
                    inventory: built.summary().source_inventory(),
                    definition,
                    kind,
                },
                &source::guid(objective),
            )?;
        }
    }
    Ok(())
}

#[test]
fn flow_wrappers_keep_absent_pointer_declarations_and_empty_arrays_distinct() -> TestResult {
    let (catalog, _) = complete(Case::AbsentAction)?;
    let graph = Template::new(&catalog, &source::guid(fixture::TEMPLATE))?;
    let node = wire::root(&graph.owner, graph.triggers()?)?;
    let entries = raw::array(wire::meaning(node, "pickRandomAction")?, "entries")?;
    let [entry] = entries else {
        return Err("one original random action entry".into());
    };
    assert_eq!(entry["type"], "declared");
    assert_eq!(
        entry["actionField"]["type"], "absent",
        "actual declaration survey"
    );
    assert_eq!(entry["action"], Value::Null, "no invented source node");
    let (catalog, _) = complete(Case::AbsentActions)?;
    let graph = Template::new(&catalog, &source::guid(fixture::TEMPLATE))?;
    let node = wire::root(&graph.owner, graph.triggers()?)?;
    let action = wire::meaning(node, "multipleActions")?;
    assert_eq!(action["actionsField"]["type"], "absent");
    assert_eq!(action["actions"], json!([]));
    Ok(())
}

#[test]
fn flow_wrappers_visit_deep_original_graphs_without_a_fixed_cutoff() -> TestResult {
    const DEPTH: usize = 96;
    let (catalog, fixture) = complete(Case::Deep(DEPTH))?;
    let graph = Template::new(&catalog, &source::guid(fixture::TEMPLATE))?;
    let mut next = wire::root(&graph.owner, graph.triggers()?)?;
    for depth in (0..DEPTH).rev() {
        let reference = if depth % 2 == 0 {
            wire::meaning(next, "multipleActions")?.pointer("/actions/0")
        } else {
            wire::meaning(next, "pickRandomAction")?.pointer("/entries/0/action")
        }
        .ok_or("every deep original action edge survives")?;
        next = graph.owner.follow(reference)?;
    }
    wire::objective(
        &graph.owner,
        next,
        &source::guid(fixture::TEMPLATE_OBJECTIVE),
    )?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let options = CatalogOptions {
        cancel: Some(&cancel),
        ..source::options()
    };
    assert!(
        matches!(
            build_catalog_from(&fixture.archive, &fixture.entry, &options),
            Err(CatalogError::Cancelled)
        ),
        "actual writer respects cancellation"
    );
    Ok(())
}

#[test]
fn flow_wrappers_keep_nested_unsupported_and_unreadable_occurrences_in_diagnostics() -> TestResult {
    for (case, expected) in [
        (Case::UnknownChild, ExportDiagnosticCode::UnsupportedValue),
        (
            Case::UnreadableChild,
            ExportDiagnosticCode::UnreadableSource,
        ),
    ] {
        let (catalog, _) = refused(case, expected)?;
        let graph = Template::new(&catalog, &source::guid(fixture::TEMPLATE))?;
        let node = wire::root(&graph.owner, graph.triggers()?)?;
        let children = raw::array(wire::meaning(node, "multipleActions")?, "actions")?;
        match case {
            Case::UnknownChild => {
                let [random] = children else {
                    return Err("unknown nested action control".into());
                };
                let nested = graph.owner.follow(random)?;
                let meaning = wire::meaning(nested, "pickRandomAction")?;
                let action = graph.owner.follow(
                    meaning
                        .pointer("/entries/0/action")
                        .ok_or("nested raw action")?,
                )?;
                wire::meaning(action, "unsupported")?;
                assert_eq!(
                    raw::payload(action, "struct")?["sourceType"]["value"],
                    fixture::MYSTERY
                );
            }
            Case::UnreadableChild => {
                let [first, error, last] = children else {
                    return Err("three authored action slots".into());
                };
                wire::meaning(graph.owner.follow(first)?, "stateChange")?;
                let error = graph.owner.follow(error)?;
                assert_eq!(error["value"]["state"], "unreadable");
                assert_eq!(error["action"], Value::Null);
                wire::meaning(graph.owner.follow(last)?, "stateChange")?;
            }
            _ => return Err("unexpected fixture".into()),
        }
    }
    Ok(())
}

#[test]
fn flow_wrappers_reject_wrong_declared_storage_and_unknown_random_entries() -> TestResult {
    for (case, code, tag) in [
        (
            Case::WrongMultipleStorage,
            ExportDiagnosticCode::MalformedSource,
            "multipleActions",
        ),
        (
            Case::WrongPickStorage,
            ExportDiagnosticCode::MalformedSource,
            "pickRandomAction",
        ),
        (
            Case::WrongEntry,
            ExportDiagnosticCode::UnsupportedValue,
            "pickRandomAction",
        ),
        (
            Case::WrongProbability,
            ExportDiagnosticCode::MalformedSource,
            "pickRandomAction",
        ),
    ] {
        let (catalog, _) = refused(case, code)?;
        let graph = Template::new(&catalog, &source::guid(fixture::TEMPLATE))?;
        let node = wire::root(&graph.owner, graph.triggers()?)?;
        let meaning = wire::meaning(node, tag)?;
        if matches!(case, Case::WrongEntry) {
            assert_eq!(meaning["entries"][0]["type"], "unsupported");
        }
        if matches!(case, Case::WrongProbability) {
            assert_eq!(
                meaning["entries"][0]["probabilityInterpretation"],
                "wrongShape"
            );
        }
    }
    Ok(())
}

#[test]
fn released_v2_action_union_remains_a_historical_boundary() -> TestResult {
    let released: Value =
        serde_json::from_str(include_str!("../fixtures/catalog-v0.4.2.schema.json"))?;
    let schema = json!({ "$ref":"#/$defs/FlowAction", "$defs":released["$defs"] });
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&json!({"type":"stateChange","toState":null})),
        "historical positive control"
    );
    for name in ["multipleActions", "pickRandomAction"] {
        assert!(
            !validator.is_valid(&json!({"type":name,"actions":[]})),
            "released closed union"
        );
    }
    Ok(())
}

fn complete(case: Case) -> TestResult<(Value, Fixture)> {
    let fixture = fixture::open(case, Input::Plain, false)?;
    let built = build_catalog_from(&fixture.archive, &fixture.entry, &source::options())?;
    assert!(
        built.summary().export_health().is_complete(),
        "readable typed action graph"
    );
    Ok((serde_json::from_slice(built.json())?, fixture))
}

fn refused(case: Case, expected: ExportDiagnosticCode) -> TestResult<(Value, Fixture)> {
    let fixture = fixture::open(case, Input::Encrypted, false)?;
    assert!(
        matches!(
            build_catalog_from(&fixture.archive, &fixture.entry, &source::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "wrong source never publishes as complete"
    );
    let (built, catalog) = fixture.diagnostic()?;
    assert!(
        built
            .summary()
            .export_health()
            .diagnostics()
            .iter()
            .any(|issue| issue.code == expected),
        "the actual source failure has its own typed diagnostic"
    );
    Ok((catalog, fixture))
}
