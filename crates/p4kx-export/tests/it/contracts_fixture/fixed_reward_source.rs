// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Field, RecordTree, StructValue, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value as Json, json};

use crate::health_origin_fixture::{self, TestResult};

use super::super::{CONTRACT_A, CONTRACT_B, GENERATOR, SUB_TWO, records};

pub(super) const MALFORMED: u128 = 0x2700_0000_0000_4000_8000_0000_0000_0001;
const WIDE_MAXIMUM: i64 = 9_007_199_254_740_993;
const MALFORMED_GENERATOR: u128 = 0x2700_0000_0000_4000_8000_0000_0000_0002;

#[derive(Clone, Copy, Debug)]
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

pub(super) struct Ordinary {
    pub(super) full_success: Site,
    pub(super) base_success: Site,
    pub(super) failure: Site,
    pub(super) currency: Site,
    pub(super) full_amount: Site,
    pub(super) maximum: Site,
    pub(super) bonuses: Site,
    pub(super) base_amount: Site,
    pub(super) base_owner: Site,
    pub(super) failed_amount: Site,
    pub(super) child_result: Site,
    pub(super) empty_results: Site,
}

pub(super) struct Malformed {
    pub(super) success: Site,
    pub(super) success_error: Site,
    pub(super) ignored_later_success: Site,
    pub(super) ignored_amount: Site,
    pub(super) amount: Site,
    pub(super) bonuses_error: Site,
    pub(super) wide_maximum: Site,
}

pub(super) fn with_malformed(database: DataForgeBuilder) -> DataForgeBuilder {
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(MALFORMED))
        .value("debugName", TestValue::text("PaymentControl"))
        .value(
            "contractResults",
            records::results(vec![
                payout(TestValue::Raw(vec![3]), 99, TestValue::Bool(true)),
                payout(TestValue::Bool(true), -9, TestValue::Raw(vec![3])),
            ]),
        );
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("debugName", TestValue::text("PaymentControl"))
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        );
    database
        .structure(
            TestStruct::new("MissionRewardWide")
                .parent("MissionReward")
                .field("max", TestType::Int64)
                .field("plusBonuses", TestType::Bool)
                .field("currencyType", TestType::Enum("ECurrency".to_owned())),
        )
        .record(
            TestRecord::new(
                MALFORMED_GENERATOR,
                "ContractGenerator",
                "ContractGenerator.PaymentControl",
                "libs/foundry/records/contracts/contractgenerator/payment_control.xml",
            )
            .value(
                "generators",
                TestValue::Array(vec![TestValue::Struct(handler)]),
            ),
        )
}

fn payout(success: TestValue, amount: i32, bonuses: TestValue) -> TestValue {
    let flag = |value| TestValue::Struct(TestInstance::new("Bool").value("value", value));
    let reward = TestInstance::new("MissionRewardWide")
        .value("reward", TestValue::Int32(amount))
        .value("max", TestValue::Int64(WIDE_MAXIMUM))
        .value("currencyType", TestValue::text("UEC"))
        .value("plusBonuses", bonuses);
    TestValue::Struct(
        TestInstance::new("ContractResult_Reward")
            .value(
                "missionResults",
                TestValue::Array(vec![flag(success), flag(TestValue::Bool(true))]),
            )
            .value("contractReward", TestValue::Struct(reward)),
    )
}

pub(super) fn ordinary(forge: &DataForge<'_>) -> TestResult<Ordinary> {
    let tree = generator(forge, GENERATOR)?;
    let handler = item(tree.root(), "generators", 0)?;
    let parent = item(handler, "contracts", 0)?;
    let without_payout = item(handler, "contracts", 1)?;
    let child = item(parent, "subContracts", 1)?;
    for (owner, guid) in [
        (parent, CONTRACT_A),
        (without_payout, CONTRACT_B),
        (child, SUB_TWO),
    ] {
        check_id(owner, guid);
    }
    let block = structure(field(parent, "contractResults")?)?;
    let full = result(block, 2)?;
    let failure = result(block, 3)?;
    let base = result(block, 4)?;
    let full_data = structure(field(full, "contractReward")?)?;
    let failed_data = structure(field(failure, "contractReward")?)?;
    let base_owner = field(base, "contractReward")?;
    let base_data = structure(base_owner)?;
    for name in ["currencyType", "max", "plusBonuses"] {
        assert!(
            base_data.field(name).is_none(),
            "base payout does not declare {name}"
        );
    }
    let child_block = structure(field(child, "contractResults")?)?;
    let child_result = slot(child_block, "contractResults", 0)?;
    assert_eq!(
        structure(child_result)?.type_name(),
        "BlueprintRewards",
        "the child has real results of its own"
    );
    let empty = structure(field(without_payout, "contractResults")?)?;
    let empty_results = field(empty, "contractResults")?;
    assert!(
        matches!(empty_results, Value::Array(entries) if entries.is_empty()),
        "the bare contract has an original empty results array"
    );
    Ok(Ordinary {
        full_success: selected(&tree, flag(full, 0)?, &Value::Bool(true))?,
        base_success: selected(&tree, flag(base, 0)?, &Value::Bool(true))?,
        failure: selected(&tree, flag(failure, 0)?, &Value::Bool(false))?,
        currency: selected(
            &tree,
            field(full_data, "currencyType")?,
            &Value::Enum("UEC"),
        )?,
        full_amount: selected(&tree, field(full_data, "reward")?, &Value::Int32(40_000))?,
        maximum: selected(&tree, field(full_data, "max")?, &Value::Int32(0))?,
        bonuses: selected(&tree, field(full_data, "plusBonuses")?, &Value::Bool(false))?,
        base_amount: selected(&tree, field(base_data, "reward")?, &Value::Int32(7))?,
        base_owner: site(&tree, base_owner)?,
        failed_amount: selected(&tree, field(failed_data, "reward")?, &Value::Int32(1_000))?,
        child_result: site(&tree, child_result)?,
        empty_results: site(&tree, empty_results)?,
    })
}

pub(super) fn malformed(forge: &DataForge<'_>) -> TestResult<Malformed> {
    let tree = generator(forge, MALFORMED_GENERATOR)?;
    let handler = item(tree.root(), "generators", 0)?;
    let contract = item(handler, "contracts", 0)?;
    check_id(contract, MALFORMED);
    let block = structure(field(contract, "contractResults")?)?;
    let rejected = result(block, 0)?;
    let retained = result(block, 1)?;
    let ignored_reward = structure(field(rejected, "contractReward")?)?;
    let retained_reward = structure(field(retained, "contractReward")?)?;
    let invalid = Value::Error(ValueError::InvalidBool { byte: 3 });
    Ok(Malformed {
        success: selected(&tree, flag(retained, 0)?, &Value::Bool(true))?,
        success_error: selected(&tree, flag(rejected, 0)?, &invalid)?,
        ignored_later_success: selected(&tree, flag(rejected, 1)?, &Value::Bool(true))?,
        ignored_amount: selected(&tree, field(ignored_reward, "reward")?, &Value::Int32(99))?,
        amount: selected(&tree, field(retained_reward, "reward")?, &Value::Int32(-9))?,
        bonuses_error: selected(&tree, field(retained_reward, "plusBonuses")?, &invalid)?,
        wide_maximum: selected(
            &tree,
            field(retained_reward, "max")?,
            &Value::Int64(WIDE_MAXIMUM),
        )?,
    })
}

fn generator<'source>(
    forge: &'source DataForge<'source>,
    guid: u128,
) -> TestResult<RecordTree<'source>> {
    let guid = health_origin_fixture::guid(guid).parse()?;
    let record = forge
        .record_by_guid(guid)
        .ok_or("original Generator Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record) && record.type_name()? == "ContractGenerator",
        "the read tree belongs to the actual Generator"
    );
    Ok(tree)
}

fn check_id(owner: &StructValue<'_>, expected: u128) {
    assert!(
        matches!(owner.field("id"), Some(Value::Guid(guid))
            if guid.to_string() == health_origin_fixture::guid(expected)),
        "the actual contract occurrence stores the controlled logical identity"
    );
}

fn result<'read, 'source>(
    block: &'read StructValue<'source>,
    index: usize,
) -> TestResult<&'read StructValue<'source>> {
    let result = item(block, "contractResults", index)?;
    assert_eq!(
        result.type_name(),
        "ContractResult_Reward",
        "the original payout slot"
    );
    Ok(result)
}

fn flag<'read, 'source>(
    result: &'read StructValue<'source>,
    index: usize,
) -> TestResult<&'read Value<'source>> {
    field(item(result, "missionResults", index)?, "value")
}

fn item<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
    index: usize,
) -> TestResult<&'read StructValue<'source>> {
    structure(slot(owner, name, index)?)
}

fn slot<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
    index: usize,
) -> TestResult<&'read Value<'source>> {
    let Value::Array(values) = field(owner, name)? else {
        return Err(format!("expected original {name} array").into());
    };
    values
        .get(index)
        .ok_or_else(|| format!("missing original {name}[{index}]").into())
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
        return Err("expected original Struct occurrence".into());
    };
    Ok(value)
}

fn selected<'source>(
    tree: &RecordTree<'source>,
    value: &Value<'source>,
    expected: &Value<'_>,
) -> TestResult<Site> {
    assert_eq!(
        value, expected,
        "the actual physical field stores this expected primitive"
    );
    site(tree, value)
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
    Err("the selected field belongs to another physical tree".into())
}
