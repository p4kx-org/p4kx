// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::BlueprintPoolRead;
use p4kx_extraction::{RecordTree, SourceValueState, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::test_support;
use crate::catalog_number_capture::FloatToken;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};

use crate::catalog_v3::industry_modules::blueprint_source_test_selectors as selectors;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::resource_number::ResourceNumber;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn database(weights: &[f32]) -> Vec<u8> {
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("RewardBase"))
        .structure(
            TestStruct::new("BlueprintReward")
                .parent("RewardBase")
                .field("blueprintRecord", TestType::Reference)
                .field("weight", TestType::Single),
        )
        .structure(
            TestStruct::new("BlueprintPoolRecord")
                .array(
                    "blueprintRewards",
                    TestType::StrongPointer("RewardBase".to_owned()),
                )
                .field("futureNumber", TestType::Double),
        );
    let entries = weights.iter().map(|weight| {
        TestValue::Struct(
            TestInstance::new("BlueprintReward")
                .value("blueprintRecord", TestValue::Reference(9))
                .value("weight", TestValue::Single(*weight)),
        )
    });
    builder
        .record(
            TestRecord::new(
                1,
                "BlueprintPoolRecord",
                "BlueprintPoolRecord.Primary",
                "libs/foundry/records/crafting/blueprintrewards/primary.xml",
            )
            .value(
                "blueprintRewards",
                TestValue::Array(std::iter::once(TestValue::Null).chain(entries).collect()),
            )
            .value(
                "futureNumber",
                TestValue::Double(f64::from_bits(0x7ff8_0000_0000_00a1)),
            ),
        )
        .build()
}

#[test]
fn original_pool_slots_and_exact_numbers_survive_the_unchanged_f32_weight_projection() -> TestResult
{
    let source = [
        1.0,
        -2.0,
        -0.0,
        f32::from_bits(0x7fc0_00a1),
        f32::INFINITY,
        3.0,
    ];
    test_support::with_same_forge_reports(&database(&source), |forge, input, _| {
        let record = forge.record(0).ok_or("original physical pool")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let read = super::read(&tree, input, &admission, None)?;
        verify_slots(&tree, &read, &source)?;
        verify_weight_summary(&read)
    })
}

fn verify_slots(tree: &RecordTree<'_>, read: &BlueprintPoolRead<'_>, source: &[f32]) -> TestResult {
    let Value::Array(original) = tree
        .root()
        .field("blueprintRewards")
        .ok_or("authored reward array")?
    else {
        return Err("original reward array storage".into());
    };
    assert_eq!(
        original.len(),
        source.len() + 1,
        "the original null slot exists"
    );
    assert!(
        matches!(original.first(), Some(Value::Null)),
        "the original first slot is null"
    );
    let entries = read
        .prepared
        .definition
        .blueprint_rewards()
        .value()
        .ok_or("typed reward slots")?;
    assert_eq!(
        entries.len(),
        original.len(),
        "no slot filtering or deduplication"
    );
    assert_eq!(
        entries[0].state(),
        SourceValueState::Null,
        "null remains a real slot"
    );
    for (entry, weight) in entries[1..].iter().zip(source) {
        let Some(ResourceNumber::Single(value)) =
            entry.value().ok_or("typed entry")?.weight().value()
        else {
            return Err("original Single weight".into());
        };
        assert_eq!(
            value.raw_bits(),
            weight.to_bits(),
            "original IEEE bits survive"
        );
    }
    Ok(())
}

fn verify_weight_summary(read: &BlueprintPoolRead<'_>) -> TestResult {
    assert_eq!(
        read.prepared
            .weights
            .total_weight()
            .value()
            .ok_or("derived sum")?
            .raw_bits(),
        read.interpreted.total_weight().to_bits(),
        "actual public library semantics are unchanged"
    );
    assert_eq!(
        read.prepared
            .weights
            .total_weight()
            .value()
            .and_then(ResourceFloat32::finite),
        Some(4.0),
        "negative, zero and nonfinite original weights contribute no positive interpreted weight"
    );
    assert!(
        read.prepared
            .weights
            .total_weight()
            .origin()
            .source_set_complete(),
        "all inspected values are accounted"
    );
    assert!(
        read.prepared
            .numbers
            .iter()
            .any(|number| number.token == FloatToken::Single(0x7fc0_00a1)),
        "raw NaN receipt"
    );
    assert!(
        read.prepared
            .numbers
            .iter()
            .any(|number| number.token == FloatToken::Double(0x7ff8_0000_0000_00a1)),
        "additional raw Double receipt"
    );
    Ok(())
}

#[test]
fn positive_f32_sum_overflow_is_retained_as_bits_instead_of_a_json_number() -> TestResult {
    test_support::with_same_forge_reports(&database(&[f32::MAX, f32::MAX]), |forge, input, _| {
        let record = forge.record(0).ok_or("original pool")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let read = super::read(&tree, input, &admission, None)?;
        let value = read
            .prepared
            .weights
            .total_weight()
            .value()
            .ok_or("weight sum")?;
        assert_eq!(
            value.raw_bits(),
            f32::INFINITY.to_bits(),
            "finite f32 contribution overflow remains exact"
        );
        assert!(
            value.finite().is_none(),
            "nonfinite derived values have no bare JSON number"
        );
        assert!(
            serde_json::to_value(&read.prepared.weights).is_ok(),
            "the projection remains valid JSON"
        );
        Ok(())
    })
}

#[test]
fn named_weight_fields_contribute_their_actual_original_selectors() -> TestResult {
    test_support::with_same_forge_reports(&database(&[1.0, -2.0]), |forge, input, _| {
        let record = forge.record(0).ok_or("original physical pool")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let read = super::read(&tree, input, &admission, None)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let rewards = fields
            .field(None, "blueprintRewards")?
            .ok_or("original array selector")?
            .1;
        selectors::contains(
            read.prepared.weights.total_weight(),
            rewards,
            OriginalValuePart::FieldName,
        )?;
        for weight in fields
            .facts()
            .iter()
            .filter(|fact| fact.field().name() == "weight")
        {
            selectors::contains(
                read.prepared.weights.total_weight(),
                weight,
                OriginalValuePart::FieldName,
            )?;
        }
        Ok(())
    })
}
