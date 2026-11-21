// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use serde_json::{Value, json};

use super::version::matches_build;
use super::{Dumps, Findings, P4kx, collect_contracts, compare};

const BLUEPRINT: &str = "00000000-0000-0000-0000-0000000000b1";
const PRODUCT: &str = "00000000-0000-0000-0000-0000000000e1";
const ELEMENT: &str = "00000000-0000-0000-0000-0000000000a1";
const RESOURCE: &str = "00000000-0000-0000-0000-0000000000a2";
const STANDING: &str = "00000000-0000-0000-0000-0000000000c1";
const CONTRACT: &str = "00000000-0000-0000-0000-0000000000d1";
const SUB_CONTRACT: &str = "00000000-0000-0000-0000-0000000000d2";

fn dumps() -> Dumps {
    Dumps {
        merged: json!({
            "contracts": [
                {"id": CONTRACT, "debugName": "Courier", "timeToComplete": 10, "buyIn": null,
                 "minStanding": {"guid": STANDING, "name": "Neutral", "minReputation": 0}},
                {"id": SUB_CONTRACT, "debugName": "Courier_0", "timeToComplete": 10}
            ],
            "legacyContracts": []
        }),
        blueprints: json!({"blueprints": [{
            "guid": BLUEPRINT, "tag": "BP_TEST", "productEntityClass": PRODUCT,
            "tiers": [{"craftTimeSeconds": 3725, "slots": [{
                "options": [{"quantity": 2.0, "minQuality": 900}],
                "modifiers": [{"startQuality": 0, "endQuality": 1000,
                               "modifierAtStart": 0.9, "modifierAtEnd": 1.1}]
            }]}]
        }]}),
        mining: json!({
            "mineableElements": {ELEMENT: {"name": "Ore", "resourceType": RESOURCE,
                                           "instability": 350.0, "density": 7.14}},
            "compositions": {},
            "clusteringPresets": {}
        }),
    }
}

fn blueprint_record(product: &Value, modifier_at_end: f64) -> Value {
    json!({"blueprint": {
        "processSpecificData": {"entityClass": product},
        "tiers": [{"recipe": {"costs": {
            "craftTime": {"hours": 1, "minutes": 2, "seconds": 5},
            "mandatoryCost": {"options": [{
                "options": [{"quantity": {"standardCargoUnits": 2.0}, "minQuality": 900}],
                "context": [{"gameplayPropertyModifiers": {"gameplayPropertyModifiers": [
                    {"valueRanges": [{"startQuality": 0, "endQuality": 1000,
                                      "additiveModifierAtStart": 0.9,
                                      "additiveModifierAtEnd": modifier_at_end}]}
                ]}}]
            }]}
        }}}]
    }})
}

fn p4kx(product: &Value, modifier_at_end: f64) -> P4kx {
    let records = HashMap::from([
        (
            BLUEPRINT.to_owned(),
            blueprint_record(product, modifier_at_end),
        ),
        (
            ELEMENT.to_owned(),
            json!({"elementInstability": 350.0, "resourceType": {"guid": RESOURCE}}),
        ),
        (STANDING.to_owned(), json!({"minReputation": 0})),
    ]);
    let mut contracts = HashMap::new();
    collect_contracts(
        &json!({"generators": [{"contracts": [{
            "id": CONTRACT, "debugName": "Courier",
            "contractResults": {"timeToComplete": 10, "contractBuyInAmount": 0},
            "minStanding": {"guid": STANDING},
            "subContracts": [{"$type": "SubContract", "id": SUB_CONTRACT}]
        }]}]}),
        &mut contracts,
    );
    P4kx { records, contracts }
}

#[test]
fn a_dump_version_matches_the_branch_and_change_number_of_its_build() {
    assert!(matches_build(
        "4.10.2-live.12913470",
        "sc-alpha-4.10.2",
        "12913470"
    ));
    assert!(!matches_build(
        "4.10.2-live.12913470",
        "sc-alpha-4.10.3",
        "12913470"
    ));
    assert!(!matches_build(
        "4.10.2-live.12913470",
        "sc-alpha-4.10.2",
        "12913471"
    ));
    assert!(!matches_build("4.10.2", "sc-alpha-4.10.2", "12913470"));
}

#[test]
fn the_record_guids_are_those_the_dumps_describe() {
    let dumps = dumps();
    let guids: Vec<&str> = dumps.record_guids().into_iter().collect();
    assert_eq!(guids, [ELEMENT, BLUEPRINT, STANDING]);
}

#[test]
fn equal_values_agree_and_texts_and_densities_are_not_compared() {
    let findings = compare(&dumps(), &p4kx(&json!({"guid": PRODUCT}), 1.1));
    assert!(findings.is_empty(), "{:?}", findings.examples(usize::MAX));
    assert!(findings.summary().len() >= 5, "{:?}", findings.summary());
}

#[test]
fn a_dangling_product_agrees_when_its_error_names_the_guid() {
    let dangling = json!({"error": format!("the Reference refers to the Record {PRODUCT}")});
    assert!(compare(&dumps(), &p4kx(&dangling, 1.1)).is_empty());
    let other = json!({"error": "the Reference refers to another Record"});
    assert_eq!(
        compare(&dumps(), &p4kx(&other, 1.1)).mismatches_of("blueprint products"),
        1
    );
}

#[test]
fn a_different_modifier_range_disagrees() {
    let findings = compare(&dumps(), &p4kx(&json!({"guid": PRODUCT}), 1.2));
    assert_eq!(findings.mismatches_of("blueprint modifiers"), 1);
    assert_eq!(findings.examples(usize::MAX).len(), 1);
}

#[test]
fn a_contract_in_no_record_disagrees() {
    let mut p4kx = p4kx(&json!({"guid": PRODUCT}), 1.1);
    p4kx.contracts.remove(SUB_CONTRACT);
    assert_eq!(compare(&dumps(), &p4kx).mismatches_of("contracts"), 1);
}

#[test]
fn numbers_agree_within_the_tolerance_of_an_f32() {
    let mut findings = Findings::default();
    findings.number("n", "x", "f", Some(0.3), Some(f64::from(0.3_f32)));
    assert!(findings.is_empty());
    findings.number("n", "x", "f", Some(1.0), None);
    findings.number("n", "x", "f", Some(1.0), Some(1.001));
    assert_eq!(findings.mismatches_of("n"), 2);
}

#[test]
fn a_field_missing_on_both_sides_is_tallied_and_not_compared() {
    let mut findings = Findings::default();
    findings.number("n", "x", "f", Some(1.0), Some(1.0));
    findings.number("n", "x", "f", None, None);
    assert!(findings.is_empty());
    assert_eq!(findings.compared("n", "f"), 1);
    assert_eq!(findings.absent_on_both("n", "f"), 1);
    assert_eq!(findings.absent_on_one_side("n", "f"), 0);
    assert!(
        findings
            .summary()
            .iter()
            .any(|line| line.contains("f: compared 1 of 2")),
        "{:?}",
        findings.summary()
    );
}

#[test]
fn an_optional_field_missing_on_one_side_is_tallied_without_a_disagreement() {
    let mut findings = Findings::default();
    findings.number_if_both("n", "x", "f", Some(1.0), None);
    findings.text_if_both("n", "x", "t", None, Some("a"));
    assert!(findings.is_empty());
    assert_eq!(findings.compared("n", "f"), 0);
    assert_eq!(findings.absent_on_one_side("n", "f"), 1);
    assert_eq!(findings.absent_on_one_side("n", "t"), 1);
}

#[test]
fn a_renamed_contract_field_shows_in_the_tally() {
    let mut p4kx = p4kx(&json!({"guid": PRODUCT}), 1.1);
    p4kx.contracts.clear();
    collect_contracts(
        &json!({"contracts": [{
            "id": CONTRACT, "contractResults": {"timeToCompleteRenamed": 10},
            "minStanding": {"guid": STANDING}
        }]}),
        &mut p4kx.contracts,
    );
    let findings = compare(&dumps(), &p4kx);
    assert_eq!(findings.compared("contracts", "timeToComplete"), 0);
    assert_eq!(
        findings.absent_on_one_side("contracts", "timeToComplete"),
        1
    );
}

#[test]
fn contract_fields_present_on_both_sides_are_compared() {
    let findings = compare(&dumps(), &p4kx(&json!({"guid": PRODUCT}), 1.1));
    assert_eq!(findings.compared("contracts", "timeToComplete"), 1);
    assert_eq!(
        findings.absent_on_one_side("contracts", "timeToComplete"),
        1
    );
    assert_eq!(findings.compared("contracts", "minStanding"), 1);
    assert_eq!(findings.absent_on_both("contracts", "minStanding"), 1);
    assert_eq!(findings.absent_on_one_side("contracts", "buyIn"), 1);
}

#[path = "tests_regressions.rs"]
mod regressions;
