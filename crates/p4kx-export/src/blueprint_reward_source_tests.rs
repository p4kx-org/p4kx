// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::source_value_catalog::SourceDataType;
use p4kx_extraction::{CancelToken, DataType, Record, RecordTree, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::BlueprintRewardSource;
use crate::build::original_input::test_support;
use crate::catalog_read::original::OriginalFlowFields;
use crate::crafting_sources::{fixture, scope_fixture};
use crate::crafting_types::RewardSource;
use crate::record_observation::RecordObservation;
use crate::resource_number::ResourceNumber;

#[path = "blueprint_reward_selector_tests.rs"]
mod selector_tests;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn original_reward_declarations_preserve_deep_order_duplicates_and_identity_precedence()
-> TestResult {
    for bytes in [fixture::database(257), scope_fixture::database()] {
        test_support::with_same_forge_reports(&bytes, |forge, input, _| {
            let record = forge.record(0).ok_or("original reward owner")?;
            let tree = record.tree()?;
            let admission = RecordObservation::capture(input, &record)?;
            let wanted = fixture::pools();
            let mut expected = Vec::new();
            crate::crafting_sources::collect(&tree, &wanted, &mut expected);
            let read = super::read(&tree, input, &admission, &wanted, None)?;
            assert_eq!(read.interpreted, expected, "shared actual source walk");
            assert_eq!(
                read.rows.len(),
                expected.len(),
                "one declaration per actual scalar occurrence"
            );
            let scope = input.record_scope(&record, &tree)?;
            let fields = OriginalFlowFields::read(&scope, None)?;
            let mut last = None;
            for (row, expected) in read.rows.iter().zip(expected) {
                let ordinal = row
                    .source_ordinal()
                    .value()
                    .ok_or("original ordinal")?
                    .get();
                assert!(
                    last.is_none_or(|last| last < ordinal),
                    "source DFS order is retained"
                );
                last = Some(ordinal);
                verify_reward_occurrence(row, &expected, &fields, ordinal)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn verify_reward_occurrence(
    row: &BlueprintRewardSource<'_>,
    expected: &RewardSource<'_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
    ordinal: u64,
) -> TestResult {
    let fact = fields
        .facts()
        .get(usize::try_from(ordinal)?)
        .ok_or("original reference fact")?;
    let Value::Reference(original) = fact.value() else {
        return Err("original reference occurrence".into());
    };
    assert_eq!(
        original.guid(),
        expected.pool,
        "the source ordinal points at the actual pool reference"
    );
    assert_eq!(
        row.pool().value().ok_or("pool link")?.guid().value(),
        Some(&expected.pool),
        "original nominal pool GUID"
    );
    assert_eq!(
        row.contract_id().value().map(|guid| guid.0),
        expected.contract_id,
        "nearest identity GUID"
    );
    assert_eq!(
        row.contract_name().value().copied(),
        expected.contract_name,
        "matching identity name"
    );
    let chance = match row.chance().value() {
        Some(ResourceNumber::Single(value)) => Some(value.raw_bits()),
        _ => None,
    };
    assert_eq!(
        chance,
        expected.chance.map(f32::to_bits),
        "unmodified local chance bits"
    );
    assert!(
        row.source_ordinal().origin().source_set_complete()
            && row.pool().origin().source_set_complete()
            && row.contract_id().origin().source_set_complete()
            && row.contract_name().origin().source_set_complete()
            && row.chance().origin().source_set_complete(),
        "each derived or retained datum closes over actual source evidence"
    );
    Ok(())
}

#[test]
fn absent_reward_targets_and_cancelled_reads_do_not_fabricate_declarations() -> TestResult {
    test_support::with_same_forge_reports(&scope_fixture::database(), |forge, input, _| {
        let record = forge.record(0).ok_or("original reward owner")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let read = super::read(
            &tree,
            input,
            &admission,
            &std::collections::HashSet::new(),
            None,
        )?;
        assert!(
            read.rows.is_empty() && read.interpreted.is_empty() && read.numbers.is_empty(),
            "no invented inverse declaration or receipt"
        );
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            matches!(
                super::read(&tree, input, &admission, &fixture::pools(), Some(&cancel)),
                Err(crate::CatalogError::Cancelled)
            ),
            "cancellation remains an outer error"
        );
        Ok(())
    })
}

#[test]
fn root_weak_pool_links_preserve_the_original_storage_and_record_identity() -> TestResult {
    test_support::with_same_forge_reports(&root_weak_database(), |forge, input, _| {
        let record = forge.record(0).ok_or("original source owner")?;
        let target = forge.record(1).ok_or("original pool target")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        verify_root_weak_source(&tree, &fields)?;
        let wanted = std::collections::HashSet::from([record.guid(), target.guid()]);
        let mut expected = Vec::new();
        crate::crafting_sources::collect(&tree, &wanted, &mut expected);
        assert_eq!(
            expected.len(),
            2,
            "the actual library walk includes the direct and root-Weak links only"
        );
        let admission = RecordObservation::capture(input, &record)?;
        let read = super::read(&tree, input, &admission, &wanted, None)?;
        assert_eq!(
            read.interpreted, expected,
            "interpreted relationship semantics stay unchanged"
        );
        verify_root_weak_rows(&read.rows, &target)
    })
}

fn root_weak_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Pool"))
        .structure(TestStruct::new("Leaf").field("number", TestType::UInt32))
        .structure(
            TestStruct::new("Owner")
                .field("direct", TestType::ReferenceTo("Pool".to_owned()))
                .field("rootWeak", TestType::WeakPointer("Pool".to_owned()))
                .field("child", TestType::StrongPointer("Leaf".to_owned()))
                .field("localWeak", TestType::WeakPointer("Leaf".to_owned()))
                .field("selfWeak", TestType::WeakPointer("Owner".to_owned()))
                .field("chance", TestType::Single),
        )
        .instance(
            "child",
            TestInstance::new("Leaf").value("number", TestValue::UInt32(7)),
        )
        .record(
            TestRecord::new(1, "Owner", "Owner.Source", "rewards/source.xml")
                .value("direct", TestValue::Reference(9))
                .value("rootWeak", TestValue::pointer("Pool.Target"))
                .value("child", TestValue::pointer("child"))
                .value("localWeak", TestValue::pointer("child"))
                .value("selfWeak", TestValue::pointer("Owner.Source"))
                .value("chance", TestValue::Single(0.25)),
        )
        .record(TestRecord::new(
            9,
            "Pool",
            "Pool.Target",
            "rewards/pool.xml",
        ))
        .build()
}

fn verify_root_weak_source(
    tree: &RecordTree<'_>,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> TestResult {
    let Value::Reference(root) = tree
        .root()
        .field("rootWeak")
        .ok_or("root Weak declaration")?
    else {
        return Err("original root Weak value".into());
    };
    let Value::Reference(local) = tree
        .root()
        .field("localWeak")
        .ok_or("local Weak declaration")?
    else {
        return Err("original local Weak value".into());
    };
    assert!(
        root.location().is_none() && local.location().is_some(),
        "the control distinguishes Record and owned-instance Weak targets"
    );
    let weak = fields
        .field(None, "rootWeak")?
        .ok_or("original root Weak fact")?
        .1;
    assert_eq!(
        weak.field().storage().data_type(),
        DataType::WeakPointer,
        "actual declaration storage"
    );
    Ok(())
}

fn verify_root_weak_rows(rows: &[BlueprintRewardSource<'_>], target: &Record<'_>) -> TestResult {
    let [direct, weak] = rows else {
        return Err("two canonical original link rows".into());
    };
    assert!(
        matches!(
            direct.pool_storage().value(),
            Some(SourceDataType::Reference)
        ),
        "direct Reference storage stays original"
    );
    assert!(
        matches!(
            weak.pool_storage().value(),
            Some(SourceDataType::WeakPointer)
        ),
        "root Weak storage stays original"
    );
    for row in rows {
        let pool = row.pool().value().ok_or("physical pool link")?;
        assert_eq!(
            pool.guid().value(),
            Some(&target.guid()),
            "actual target GUID"
        );
        assert_eq!(
            pool.target_record_index().value(),
            Some(&target.index()),
            "actual target physical position"
        );
        assert!(
            row.pool().origin().source_set_complete()
                && row.pool_storage().origin().source_set_complete(),
            "conversion and original declaration each retain a closed source set"
        );
    }
    Ok(())
}
