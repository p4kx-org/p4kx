// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Original Broker reputation requirements and result amounts.

use serde_json::{Value, json};

use crate::catalog_contract_templates::build;
use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::mission_template_graph as mission;
use crate::template_canonical_graph::{BrokerContext, Context};
use crate::template_fixture_fields as fields;

use super::{BROKER, MISSION_TYPE, OWNER, SCOPE, checks};

#[test]
fn the_reputation_requirements_are_an_expression_and_the_results_name_their_amounts() -> TestResult
{
    let catalog = build()?;
    let graph = BrokerContext::new(&catalog, BROKER)?;
    requirements(&graph)?;
    reputation_results(&graph)
}

fn requirements(graph: &BrokerContext<'_>) -> TestResult {
    let terms = raw::array(graph.projection(), "standingRequirements")?;
    let block = graph.root("reputationRequirements")?;
    let original = fields::elements(graph, graph.field(block, "expression")?)?;
    let expected = [
        "requirement",
        "and",
        "leftParenthesis",
        "requirement",
        "rightParenthesis",
    ];
    assert_eq!(
        original.len(),
        expected.len(),
        "every actual original infix position"
    );
    assert_eq!(
        terms.len(),
        expected.len(),
        "the complete selected expression"
    );
    for (index, ((term, original), expected)) in
        terms.iter().zip(original).zip(expected).enumerate()
    {
        let source = graph.owner().follow(
            term.get("source")
                .ok_or("actual original term navigation")?,
        )?;
        assert!(
            std::ptr::eq(source, original),
            "the term is linked in original infix order"
        );
        assert_eq!(
            term.pointer("/operator/value"),
            Some(&json!(expected)),
            "the same standing operator"
        );
        if expected == "requirement" {
            comparison(graph, source, term, index)?;
        } else {
            assert_eq!(
                term.get("comparison"),
                Some(&Value::Null),
                "non-requirement has no comparison"
            );
        }
    }
    Ok(())
}

fn comparison(graph: &BrokerContext<'_>, source: &Value, term: &Value, index: usize) -> TestResult {
    let original = fields::text(graph.field(source, "comparison")?)?;
    let (label, expected) = match index {
        0 => ("GreaterThanOrEqualTo", json!("greaterThanOrEqualTo")),
        3 => ("Sideways", Value::Null),
        _ => return Err("the two actual original requirement positions".into()),
    };
    assert_eq!(
        original, label,
        "the exact authored comparison at its original position"
    );
    assert_eq!(
        term.pointer("/comparison/value"),
        Some(&expected),
        "the established comparison meaning"
    );
    fields::reference(graph.field(source, "factionReputation")?, OWNER)?;
    mission::reference(graph.field(source, "reputationScope")?, SCOPE, None)?;
    mission::reference(graph.field(source, "standing")?, MISSION_TYPE, None)
}

fn reputation_results(graph: &BrokerContext<'_>) -> TestResult {
    let results = raw::array(graph.projection(), "reputationResults")?;
    let [first, second] = results else {
        return Err("both original reputation results".into());
    };
    let original = fields::elements(graph, graph.root("missionResultReputationRewards")?)?;
    let [first_source, second_source] = original.as_slice() else {
        return Err("both original result positions".into());
    };
    for (row, source) in [(first, *first_source), (second, *second_source)] {
        let selected = graph.owner().follow(
            row.get("source")
                .ok_or("actual original result navigation")?,
        )?;
        assert!(
            std::ptr::eq(selected, source),
            "the result retains its actual original position"
        );
    }
    assert!(
        raw::array(second, "amounts")?.is_empty(),
        "the second result has no amount"
    );
    assert!(
        raw::elements(graph.field(second_source, "reputationAmounts")?)?.is_empty(),
        "the actual second array is empty"
    );
    checks::reputation_amount(graph, first, first_source)
}
