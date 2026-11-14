// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Field, Record, RecordTree, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType, TestValue};
use serde_json::{Value as Json, json};

use crate::health_origin_fixture::{self, TestResult};

use super::super::{
    AFFINITY, CONTRACT_A, CONTRACT_B, FACTION, FACTION_SCOPE, GENERATOR, LOSS, RANK_FIFTY,
    RANK_TWO_HUNDRED, SUB_ONE, SUB_TWO, lookups,
};

const WIDE: i64 = 9_007_199_254_740_993;
const WIDE_TYPE: &str = "ReputationRewardWide";
const MISSING_TYPE: &str = "ReputationRewardWithoutAmount";
const REWARD_PATH: &str = "reputation/rewards/missionrewards_reputation";

#[derive(Debug, Clone, Copy)]
pub(super) struct Site {
    record: u32,
    ordinal: u64,
}

impl Site {
    pub(super) fn matches(self, used: &Json) -> bool {
        used.pointer("/location/namespace").and_then(Json::as_str) == Some("dataforgeValue")
            && used.pointer("/location/indices")
                == Some(&json!([self.record.to_string(), self.ordinal.to_string()]))
    }
}

pub(super) struct Link {
    pub(super) caller: Site,
    pub(super) target: u32,
}

pub(super) struct Gain {
    pub(super) faction: Link,
    pub(super) scope: Link,
    pub(super) reward: Link,
    pub(super) amount: Site,
}

pub(super) struct Original {
    pub(super) gains: [Gain; 2],
    pub(super) success: Site,
    pub(super) failed_outcome: Site,
    pub(super) rejected_amount: Site,
    pub(super) first_reward: Site,
    pub(super) later_empty: u32,
    pub(super) child_results: Site,
    pub(super) bare_results: Site,
}

pub(super) fn with_duplicate_reward(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new(WIDE_TYPE).field("reputationAmount", TestType::Int64))
        .structure(TestStruct::new(MISSING_TYPE).field("control", TestType::Bool))
        .record(
            lookups::plain_record(
                RANK_TWO_HUNDRED,
                WIDE_TYPE,
                &format!("{REWARD_PATH}/wide_consumer"),
            )
            .value("reputationAmount", TestValue::Int64(WIDE)),
        )
        .record(lookups::plain_record(
            RANK_TWO_HUNDRED,
            MISSING_TYPE,
            &format!("{REWARD_PATH}/empty_consumer"),
        ))
}

pub(super) fn read(forge: &DataForge<'_>) -> TestResult<Original> {
    let generator = record(forge, GENERATOR)?;
    let tree = generator.tree()?;
    assert!(
        tree.is_from(&generator) && generator.type_name()? == "ContractGenerator",
        "the result slots come from their actual Generator"
    );
    let handler = item(tree.root(), "generators", 0)?;
    let parent = item(handler, "contracts", 0)?;
    let bare = item(handler, "contracts", 1)?;
    let inheriting = item(parent, "subContracts", 0)?;
    let child = item(parent, "subContracts", 1)?;
    for (source, id) in [
        (parent, CONTRACT_A),
        (bare, CONTRACT_B),
        (inheriting, SUB_ONE),
        (child, SUB_TWO),
    ] {
        assert!(
            matches!(source.field("id"), Some(Value::Guid(value))
                if value.to_string() == health_origin_fixture::guid(id)),
            "the exact authored contract id identifies each fixture row"
        );
    }

    empty_results(&tree, inheriting)?;
    original_gains(forge, &tree, parent, bare, child)
}

fn original_gains<'source>(
    forge: &DataForge<'source>,
    tree: &RecordTree<'source>,
    parent: &StructValue<'source>,
    bare: &StructValue<'source>,
    child: &StructValue<'source>,
) -> TestResult<Original> {
    let block = structure(field(parent, "contractResults")?)?;
    let success = reputation_result(block, 0)?;
    let failure = reputation_result(block, 1)?;
    let good_flag = outcome(tree, success, true)?;
    let bad_flag = outcome(tree, failure, false)?;
    let first = item(success, "contractResultReputationAmounts", 0)?;
    let second = item(success, "contractResultReputationAmounts", 1)?;
    let excluded = item(failure, "contractResultReputationAmounts", 0)?;
    let failed_reward = linked(tree, excluded, "reward", LOSS)?;
    assert_eq!(
        failed_reward.target,
        record(forge, LOSS)?.index(),
        "actual rejected reward"
    );
    let (first_reward, wide, later_empty) = duplicates(forge)?;
    let second_reward = reward_amount(&record(forge, RANK_FIFTY)?, &Value::Int32(50))?;
    let first = gain(tree, first, FACTION_SCOPE, RANK_TWO_HUNDRED, wide)?;
    let second = gain(tree, second, AFFINITY, RANK_FIFTY, second_reward)?;
    assert_eq!(
        first.reward.target, first_reward.record,
        "the actual decoder Reference selects the first physical Reward copy"
    );
    Ok(Original {
        gains: [first, second],
        success: good_flag,
        failed_outcome: bad_flag,
        rejected_amount: reward_amount(&record(forge, LOSS)?, &Value::Int32(-100))?,
        first_reward,
        later_empty,
        child_results: child_results(tree, child)?,
        bare_results: empty_results(tree, bare)?,
    })
}

fn duplicates(forge: &DataForge<'_>) -> TestResult<(Site, Site, u32)> {
    let first = record(forge, RANK_TWO_HUNDRED)?;
    let wide = forge
        .records()
        .find(|record| record.type_name().is_ok_and(|name| name == WIDE_TYPE))
        .ok_or("original wide Reward Record")?;
    let empty = forge
        .records()
        .find(|record| record.type_name().is_ok_and(|name| name == MISSING_TYPE))
        .ok_or("original empty Reward Record")?;
    assert!(
        first.guid() == wide.guid() && wide.guid() == empty.guid(),
        "all three distinct Reward Records have one nominal GUID"
    );
    assert!(
        first.index() < wide.index() && wide.index() < empty.index(),
        "reverse physical selection reaches a readable absence, then the wide amount"
    );
    assert!(
        empty.tree()?.field("reputationAmount").is_none(),
        "the final physical copy genuinely lacks the amount declaration"
    );
    Ok((
        reward_amount(&first, &Value::Int32(200))?,
        reward_amount(&wide, &Value::Int64(WIDE))?,
        empty.index(),
    ))
}

fn gain(
    tree: &RecordTree<'_>,
    entry: &StructValue<'_>,
    scope: u128,
    reward: u128,
    amount: Site,
) -> TestResult<Gain> {
    Ok(Gain {
        faction: linked(tree, entry, "factionReputation", FACTION)?,
        scope: linked(tree, entry, "reputationScope", scope)?,
        reward: linked(tree, entry, "reward", reward)?,
        amount,
    })
}

fn linked<'source>(
    tree: &RecordTree<'source>,
    entry: &StructValue<'source>,
    name: &str,
    id: u128,
) -> TestResult<Link> {
    let value = field(entry, name)?;
    let Value::Reference(reference) = value else {
        return Err(format!("expected original whole-Record Reference {name}").into());
    };
    let target = reference.original_target().record()?;
    assert!(
        reference.location().is_none()
            && target.guid().to_string() == health_origin_fixture::guid(id),
        "the actual source {name} selects its nominal target"
    );
    Ok(Link {
        caller: site(tree, value)?,
        target: target.index(),
    })
}

fn reward_amount(record: &Record<'_>, expected: &Value<'_>) -> TestResult<Site> {
    let tree = record.tree()?;
    assert!(tree.is_from(record), "actual admitted Reward Record body");
    let amount = field(tree.root(), "reputationAmount")?;
    assert_eq!(
        amount, expected,
        "exact original reward amount and storage width"
    );
    site(&tree, amount)
}

fn outcome<'source>(
    tree: &RecordTree<'source>,
    result: &StructValue<'source>,
    expected: bool,
) -> TestResult<Site> {
    let value = field(item(result, "missionResults", 0)?, "value")?;
    assert_eq!(
        value,
        &Value::Bool(expected),
        "the actual first mission outcome"
    );
    site(tree, value)
}

fn child_results<'source>(
    tree: &RecordTree<'source>,
    child: &StructValue<'source>,
) -> TestResult<Site> {
    let block = structure(field(child, "contractResults")?)?;
    let results = field(block, "contractResults")?;
    assert_eq!(
        item(block, "contractResults", 0)?.type_name(),
        "BlueprintRewards",
        "the child owns a nonempty result block of a different kind"
    );
    site(tree, results)
}

fn empty_results<'source>(
    tree: &RecordTree<'source>,
    bare: &StructValue<'source>,
) -> TestResult<Site> {
    let block = structure(field(bare, "contractResults")?)?;
    let results = field(block, "contractResults")?;
    assert!(
        matches!(results, Value::Array(items) if items.is_empty()),
        "the bare contract really has an empty result list"
    );
    site(tree, results)
}

fn reputation_result<'read, 'source>(
    block: &'read StructValue<'source>,
    index: usize,
) -> TestResult<&'read StructValue<'source>> {
    let result = item(block, "contractResults", index)?;
    assert_eq!(
        result.type_name(),
        "ContractResult_LegacyReputation",
        "actual original result kind"
    );
    Ok(result)
}

fn record<'source>(forge: &'source DataForge<'source>, guid: u128) -> TestResult<Record<'source>> {
    let guid = health_origin_fixture::guid(guid).parse()?;
    forge
        .record_by_guid(guid)
        .ok_or_else(|| "actual original Record".into())
}

fn item<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
    index: usize,
) -> TestResult<&'read StructValue<'source>> {
    let Value::Array(items) = field(owner, name)? else {
        return Err(format!("expected original {name} array").into());
    };
    structure(items.get(index).ok_or("actual original array member")?)
}

fn field<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read Value<'source>> {
    owner
        .field(name)
        .ok_or_else(|| format!("missing original {name}").into())
}

fn structure<'read, 'source>(
    value: &'read Value<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Value::Struct(value) = value else {
        return Err("expected original Struct".into());
    };
    Ok(value)
}

fn site<'source>(tree: &RecordTree<'source>, selected: &Value<'source>) -> TestResult<Site> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, selected) {
            return Ok(Site {
                record: tree.record_index(),
                ordinal,
            });
        }
        ordinal += 1;
        match value {
            Value::Struct(value) => pending.extend(value.fields().iter().rev().map(Field::value)),
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("the value is not part of its stated original tree".into())
}
