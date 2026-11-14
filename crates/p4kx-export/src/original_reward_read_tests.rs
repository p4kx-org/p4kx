// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceInventory, SourceLocation, SourceUseRole, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value as Json;

use super::RewardRead;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

const GUID: u128 = 0xd910_0000_0000_4000_8000_0000_0000_0001;

#[test]
fn physical_reward_selection_rejects_equal_guid_alternative_and_defers_payload() -> TestResult {
    test_support::with_input(&fixture(), |forge, input| {
        let first = forge.record(0).ok_or("actual first reward")?;
        let second = forge.record(1).ok_or("actual second reward")?;
        let caller = forge.record(2).ok_or("actual reward reference caller")?;
        assert_eq!(
            first.guid(),
            second.guid(),
            "real alternative records share the same nominal GUID"
        );
        let left = first.tree()?;
        let right = second.tree()?;
        let caller_tree = caller.tree()?;
        assert_original_amounts(&left, &right);
        let scope = input.record_scope(&caller, &caller_tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let (_, fact) = fields
            .field(None, "reward")?
            .ok_or("actual original reference")?;
        let observation = fact.reference_observation()?;
        let target = observation.record()?;
        assert_eq!(
            target.guid(),
            first.guid(),
            "the decoder selected an actual nominal alternative"
        );
        let a = RewardRead::from_tree(
            input,
            &RecordObservation::capture(input, &first)?,
            &left,
            None,
        )?;
        let b = RewardRead::from_tree(
            input,
            &RecordObservation::capture(input, &second)?,
            &right,
            None,
        )?;
        let before = input.origins().uses.freeze()?;
        assert_unclaimed_amounts(&before);
        let (chosen, wrong, value) = match target.index() {
            0 => (&a, &b, "2"),
            1 => (&b, &a, "9"),
            _ => return Err("the actual decoder target is one of the two original rewards".into()),
        };
        assert!(
            matches!(
                wrong.retain(input, &observation),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "matching nominal GUID cannot authorize another original physical amount"
        );
        let amount = serde_json::to_value(chosen.retain(input, &observation)?)?;
        assert_amount_value(&amount, value);
        let after = input.origins().uses.freeze()?;
        assert_amount_uses(&amount, &after, target.index(), caller.index())
    })
}

fn assert_original_amounts(left: &RecordTree<'_>, right: &RecordTree<'_>) {
    assert_eq!(
        left.field("reputationAmount"),
        Some(&Value::Int64(2)),
        "actual first amount"
    );
    assert_eq!(
        right.field("reputationAmount"),
        Some(&Value::Int64(9)),
        "actual alternative amount"
    );
}

fn assert_unclaimed_amounts(before: &SourceInventory) {
    assert!(
        before.uses().iter().all(|used| !matches!(
            used.location(), SourceLocation::Subresource { namespace, .. }
                if namespace == "dataforgeValue" && used.role() == SourceUseRole::Payload
        )),
        "no reward datum is claimed merely by retaining the original read recipe"
    );
}

fn assert_amount_value(amount: &Json, value: &str) {
    assert_eq!(
        amount.get("value"),
        Some(&serde_json::json!(value)),
        "selected exact original decimal amount"
    );
}

fn assert_amount_uses(
    amount: &Json,
    after: &SourceInventory,
    target: u32,
    caller: u32,
) -> TestResult {
    let at = |record| SourceLocation::Subresource {
        namespace: "dataforgeValue".to_owned(),
        indices: vec![u64::from(record), 0],
        field: "$value".to_owned(),
    };
    let uses = [
        (SourceUseRole::Payload, at(target)),
        (SourceUseRole::Conversion, at(target)),
        (SourceUseRole::Selector, at(caller)),
    ];
    for (role, location) in uses {
        let observed = after
            .uses()
            .iter()
            .find(|used| used.location() == &location && used.role() == role)
            .ok_or("actual closed original amount role")?;
        assert!(
            amount
                .get("sourceUsesRefs")
                .and_then(Json::as_array)
                .is_some_and(|refs| refs.contains(&serde_json::json!(observed.id().as_str()))),
            "the derived amount retains its actual selected evidence"
        );
    }
    assert_eq!(
        amount
            .get("sourceUsesRefs")
            .and_then(Json::as_array)
            .map(Vec::len),
        Some(3),
        "no other nominal alternative contributes"
    );
    Ok(())
}

fn fixture() -> Vec<u8> {
    let reward = |name, path, amount| {
        TestRecord::new(GUID, "ReputationReward", name, path)
            .value("reputationAmount", TestValue::Int64(amount))
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ReputationReward").field("reputationAmount", TestType::Int64))
        .structure(TestStruct::new("MissionBrokerEntry").field("reward", TestType::Reference))
        .record(reward(
            "ReputationReward.First",
            "synthetic/rewards/first.xml",
            2,
        ))
        .record(reward(
            "ReputationReward.Second",
            "synthetic/rewards/second.xml",
            9,
        ))
        .record(
            TestRecord::new(
                GUID + 1,
                "MissionBrokerEntry",
                "MissionBrokerEntry.Actual",
                "synthetic/brokers/actual.xml",
            )
            .value("reward", TestValue::Reference(GUID)),
        )
        .build()
}
