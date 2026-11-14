// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, ExportHealthStatus};
use serde_json::{Value, json};

use crate::flow_action_fixture::{self as fixture, Case, Names};
use crate::flow_action_source::{self as source, Step};
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;

#[path = "flow_action_wire.rs"]
mod wire;

#[test]
fn all_action_target_states_have_independent_original_source_controls() -> TestResult {
    for case in [
        Case::Single,
        Case::EqualValues,
        Case::Double,
        Case::Nonfinite,
        Case::UnknownAction,
        Case::NilEmpty,
        Case::Absent,
        Case::Null,
        Case::WrongShape,
        Case::Unreadable,
    ] {
        for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
            for names in [Names::Distinct, Names::Duplicate] {
                let original = fixture::open(case, names, input)?;
                let positions = source::check(&original, case, names, input)?;
                assert!(
                    positions.ordinal(&[Step::Field(2)])? > positions.ordinal(&[Step::Field(1)])?,
                    "later original target follows the actual Flow subtree"
                );
            }
        }
    }
    Ok(())
}

#[test]
fn selected_objective_uses_the_late_original_instance_field() -> TestResult {
    for input in originals()? {
        let catalog = catalog(&input)?;
        let graph = Template::new(&catalog, fixture::OWNER)?;
        assert_eq!(
            Some(wire::objective(
                &graph,
                wire::operation(&graph, 1, "condition")?,
                "condition"
            )?),
            Some(&json!(fixture::AFTER)),
            "the actual target id is neither the first equal-text location nor the owner header GUID"
        );
        assert_eq!(
            Some(wire::objective(
                &graph,
                wire::operation(&graph, 1, "action")?,
                "action"
            )?),
            Some(&json!(fixture::BEFORE)),
            "the actual early target positive remains distinct"
        );
    }
    Ok(())
}

#[test]
fn selected_property_uses_the_late_original_instance_field() -> TestResult {
    for input in originals()? {
        let catalog = catalog(&input)?;
        let graph = Template::new(&catalog, fixture::OWNER)?;
        assert_eq!(
            raw::payload(
                graph.bound(
                    wire::operation(&graph, 2, "condition")?
                        .pointer("/condition/property/field")
                        .ok_or("selected property field")?
                )?,
                "string"
            )?
            .pointer("/value/sourceText"),
            Some(&json!(fixture::LATE_VARIABLE)),
            "real late original variable bytes, including whitespace"
        );
        assert_eq!(
            raw::payload(
                graph.field(wire::trigger(&graph, 0)?, "description")?,
                "string"
            )?
            .pointer("/value/sourceText"),
            Some(&json!(fixture::NOTE)),
            "actual trigger note remains inert and complete"
        );
        assert_eq!(
            wire::trigger(&graph, 1)?.pointer("/trigger/noteInterpretation"),
            Some(&json!("empty")),
            "the existing nullable interpretation of an actual empty source note"
        );
        assert_eq!(
            raw::payload(
                graph.field(wire::trigger(&graph, 1)?, "description")?,
                "string"
            )?
            .pointer("/value/sourceText"),
            Some(&json!("")),
            "the actual empty source note is preserved independently of interpretation"
        );
    }
    Ok(())
}

#[test]
fn selected_random_phase_uses_the_late_original_instance_field() -> TestResult {
    for input in originals()? {
        let catalog = catalog(&input)?;
        let graph = Template::new(&catalog, fixture::OWNER)?;
        let phases = wire::phases(&graph)?;
        assert_eq!(
            phases.len(),
            2,
            "the real null phase is retained separately from the existing Struct selection"
        );
        assert_eq!(
            Some(wire::objective(
                &graph,
                phases.first().ok_or("actual first phase")?,
                "randomPhase"
            )?),
            Some(&json!(fixture::BEFORE)),
            "first real phase target positive"
        );
        assert_eq!(
            Some(wire::objective(
                &graph,
                phases.last().ok_or("actual last phase")?,
                "randomPhase"
            )?),
            Some(&json!(fixture::AFTER)),
            "last real phase target uses original positions"
        );
    }
    Ok(())
}

#[test]
fn actual_double_probability_has_the_existing_finite_projection_control() -> TestResult {
    let input = fixture::open(Case::Double, Names::Distinct, Input::Plain)?;
    source::check(&input, Case::Double, Names::Distinct, Input::Plain)?;
    let catalog = catalog(&input)?;
    let graph = Template::new(&catalog, fixture::OWNER)?;
    let phases = wire::phases(&graph)?;
    assert_eq!(
        wire::probability(
            &graph,
            phases.first().ok_or("actual selected first phase")?,
            0.25_f32
        )?
        .to_bits(),
        0.25_f64.to_bits(),
        "existing f32 normalization remains an explicit recipe alongside exact original Double bits"
    );
    Ok(())
}

fn originals() -> TestResult<Vec<Fixture>> {
    let mut inputs = Vec::new();
    for names in [Names::Distinct, Names::Duplicate] {
        let input = fixture::open(Case::Single, names, Input::Plain)?;
        source::check(&input, Case::Single, names, Input::Plain)?;
        inputs.push(input);
    }
    Ok(inputs)
}

fn catalog(input: &Fixture) -> TestResult<Value> {
    let built = p4kx_export::build_catalog_from(
        &input.archive,
        &input.entry,
        &CatalogOptions::new(UNIX_EPOCH),
    )?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "all actual source data and known shallow actions are readable"
    );
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        catalog.pointer("/definitions/contractTemplates/rows/0/guid/value"),
        Some(&json!(fixture::OWNER)),
        "the expected actual original row is selected before behavioral assertions"
    );
    Ok(catalog)
}
