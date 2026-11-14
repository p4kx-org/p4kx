// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use serde_json::{Value, json};

use super::super as comparison;
use super::super::{Dumps, P4kx};
use super::{CONTRACT, PRODUCT, STANDING, SUB_CONTRACT};

const FACTION: &str = "00000000-0000-0000-0000-0000000000f1";
const SCOPE: &str = "00000000-0000-0000-0000-0000000000f2";
const REWARD: &str = "00000000-0000-0000-0000-0000000000f3";

fn empty_dumps() -> Dumps {
    Dumps {
        merged: json!({"contracts": [], "legacyContracts": []}),
        blueprints: json!({"blueprints": []}),
        mining: json!({
            "mineableElements": {},
            "compositions": {},
            "clusteringPresets": {}
        }),
    }
}

fn contract(buy_in: i64, results: &Value) -> Value {
    json!({
        "id": CONTRACT,
        "contractResults": {
            "timeToComplete": 10,
            "contractBuyInAmount": buy_in,
            "contractResults": results
        },
        "minStanding": {"guid": STANDING},
        "subContracts": [{"$type": "SubContract", "id": SUB_CONTRACT}]
    })
}

fn replace_contracts(source: &mut P4kx, contracts: &Value) {
    source.contracts.clear();
    comparison::collect_contracts(contracts, &mut source.contracts);
}

#[test]
fn malformed_required_containers_do_not_claim_clean_comparison() {
    let cases = [
        Dumps {
            merged: json!({}),
            blueprints: json!({}),
            mining: json!({}),
        },
        Dumps {
            merged: json!({"contracts": {}, "legacyContracts": {}}),
            blueprints: json!({"blueprints": {}}),
            mining: json!({
                "mineableElements": [],
                "compositions": [],
                "clusteringPresets": []
            }),
        },
        Dumps {
            merged: json!(null),
            blueprints: json!([]),
            mining: json!(false),
        },
    ];
    let source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    for reference in cases {
        assert!(
            !comparison::compare(&reference, &source).is_empty(),
            "missing or wrong-shaped required containers need invalid-input evidence"
        );
    }
}

#[test]
fn valid_empty_domains_are_clean_and_report_zero_coverage_for_each_family() {
    let source = P4kx {
        records: HashMap::new(),
        contracts: HashMap::new(),
    };
    let findings = comparison::compare(&empty_dumps(), &source);
    assert!(findings.is_empty(), "typed empty domains are valid input");
    let summary = findings.summary();
    for (category, field) in [
        ("blueprints", "craftTimeSeconds"),
        ("contracts", "buyIn"),
        ("legacy contracts", "buyIn"),
        ("mining elements", "instability"),
        ("mining compositions", "minimumDistinctElements"),
        ("mining cluster presets", "probabilityOfClustering"),
        ("standings", "minReputation"),
    ] {
        assert_eq!(findings.compared(category, field), 0);
        assert!(
            summary
                .iter()
                .any(|line| line.starts_with(&format!("{category}:"))),
            "{category} requires an explicit zero-coverage summary"
        );
    }
}

#[test]
fn adjacent_buy_in_integers_keep_exact_identity() {
    for (expected, different) in [
        (16_777_216_i64, 16_777_217_i64),
        (9_007_199_254_740_992_i64, 9_007_199_254_740_993_i64),
    ] {
        let mut reference = super::dumps();
        reference.merged["contracts"][0]["buyIn"] = json!(expected);
        let mut source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
        replace_contracts(&mut source, &contract(different, &json!([])));
        let findings = comparison::compare(&reference, &source);
        assert_eq!(
            findings.mismatches_of("contracts"),
            1,
            "integer money must not use f32 tolerance or conversion through f64"
        );
    }
}

#[test]
fn adjacent_standing_integers_disagree() {
    let mut reference = super::dumps();
    reference.merged["contracts"][0]["minStanding"]["minReputation"] = json!(16_777_216_i64);
    let mut source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    source.records.insert(
        STANDING.to_owned(),
        json!({"minReputation": 16_777_217_i64}),
    );
    assert_eq!(
        comparison::compare(&reference, &source).mismatches_of("standings"),
        1,
        "standing thresholds are source integers"
    );
}

#[test]
fn conflicting_duplicate_contract_ids_are_not_resolved_by_order() {
    let mut reference = super::dumps();
    reference.merged["contracts"][0]["buyIn"] = json!(0);
    for buy_ins in [[0, 1], [1, 0]] {
        let mut source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
        let first = contract(buy_ins[0], &json!([]));
        let second = contract(buy_ins[1], &json!([]));
        replace_contracts(&mut source, &json!({"contracts": [first, second]}));
        assert!(
            !comparison::compare(&reference, &source).is_empty(),
            "conflicting authored occurrences must remain visible in both orders"
        );
    }
}

#[test]
fn a_success_reputation_pool_mutation_is_not_silently_accepted() {
    let mut reference = super::dumps();
    reference.merged["factionRewardsPools"] = json!([[{
        "factionGuid": FACTION, "scopeGuid": SCOPE, "amount": 40
    }]]);
    reference.merged["contracts"][0]["factionRewardsIndex"] = json!(0);
    let mut source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    replace_contracts(
        &mut source,
        &contract(
            0,
            &json!([{
                "$type": "ContractResult_LegacyReputation",
                "missionResults": [true],
                "contractResultReputationAmounts": [{
                    "factionReputation": {"guid": FACTION},
                    "reputationScope": {"guid": SCOPE},
                    "reward": {"guid": REWARD}
                }]
            }]),
        ),
    );
    source
        .records
        .insert(REWARD.to_owned(), json!({"reputationAmount": 41}));
    assert!(
        !comparison::compare(&reference, &source).is_empty(),
        "pool-bound reputation must be compared or explicitly unsupported"
    );
}

#[test]
fn failure_reputation_with_unproven_outcome_mapping_is_not_silently_accepted() {
    let mut reference = super::dumps();
    reference.merged["contracts"][0]["factionRewards_fail"] = json!([{
        "factionGuid": FACTION, "scopeGuid": SCOPE, "amount": -40
    }]);
    let mut source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    replace_contracts(
        &mut source,
        &contract(
            0,
            &json!([{
                "$type": "ContractResult_LegacyReputation",
                "missionResults": [false],
                "contractResultReputationAmounts": [{
                    "factionReputation": {"guid": FACTION},
                    "reputationScope": {"guid": SCOPE},
                    "reward": {"guid": REWARD}
                }]
            }]),
        ),
    );
    source
        .records
        .insert(REWARD.to_owned(), json!({"reputationAmount": -41}));
    assert!(
        !comparison::compare(&reference, &source).is_empty(),
        "an unproven failure mapping needs explicit uncomparable evidence"
    );
}

#[test]
fn fixed_money_with_unproven_reference_mapping_is_not_silently_accepted() {
    let mut reference = super::dumps();
    reference.merged["contracts"][0]["rewardUEC"] = json!(40);
    let mut source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    replace_contracts(
        &mut source,
        &contract(
            0,
            &json!([{
                "$type": "ContractResult_Reward",
                "missionResults": [true],
                "contractReward": {"currencyType": "UEC", "reward": 41, "max": 0, "plusBonuses": false}
            }]),
        ),
    );
    assert!(
        !comparison::compare(&reference, &source).is_empty(),
        "fixed money must disagree after a proven mapping or remain explicitly uncomparable"
    );
}

#[test]
fn explicit_empty_success_rewards_override_a_pool_reference() {
    let mut reference = super::dumps();
    reference.merged["factionRewardsPools"] = json!([[{
        "factionGuid": FACTION, "scopeGuid": SCOPE, "amount": 40
    }]]);
    reference.merged["contracts"][0]["factionRewardsIndex"] = json!(0);
    reference.merged["contracts"][0]["factionRewards"] = json!([]);
    let source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    assert!(
        comparison::compare(&reference, &source).is_empty(),
        "an explicit empty direct array takes precedence over the pool"
    );
}

#[test]
fn invalid_reputation_pool_indices_are_input_errors() {
    for index in [json!(-1), json!(0.5), json!(1), json!(u64::MAX)] {
        let mut reference = super::dumps();
        reference.merged["factionRewardsPools"] = json!([[]]);
        reference.merged["contracts"][0]["factionRewardsIndex"] = index;
        let source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
        assert_eq!(
            comparison::compare(&reference, &source).mismatches_of("reference input"),
            1,
            "a pool index must be an in-range non-negative integer"
        );
    }
}

#[test]
fn unproven_reward_mapping_is_distinct_from_a_value_disagreement() {
    let mut reference = super::dumps();
    reference.merged["contracts"][0]["rewardUEC"] = json!(40);
    let source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    let findings = comparison::compare(&reference, &source);
    assert!(
        !findings.is_empty(),
        "the comparison has explicitly uncomparable coverage"
    );
    assert!(
        !findings.has_mismatches(),
        "unproven mapping is not evidence of differing values"
    );
    assert!(
        findings
            .summary()
            .iter()
            .any(|line| line.contains("rewardUEC: not compared 1")),
        "the uncomparable occurrence must be visible in the summary"
    );
}

#[test]
fn identical_contract_aliases_do_not_create_a_conflict() {
    let mut reference = super::dumps();
    reference.merged["contracts"][0]["buyIn"] = json!(0);
    let mut source = super::p4kx(&json!({"guid": PRODUCT}), 1.1);
    replace_contracts(
        &mut source,
        &json!({"contracts": [
            contract(0, &json!([])), contract(0, &json!([]))
        ]}),
    );
    assert!(
        comparison::compare(&reference, &source).is_empty(),
        "equal authored projections may share an ID without an invented disagreement"
    );
}
