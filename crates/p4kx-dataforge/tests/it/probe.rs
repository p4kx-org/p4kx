// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The pointer probe agrees with the Record tree and never hangs on corrupt arrays.

use std::error::Error;

use p4kx_dataforge::{DataForge, DataForgeError, Record, TreeBuilder, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

#[path = "probe_strict.rs"]
mod strict;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const COMPONENT: &str = "CrafterComponentParams";
const CRAFTER_STRUCT: u32 = 1;
const OTHER: &str = "OtherParams";
const OTHER_STRUCT: u32 = 2;
const UNPOPULATED: &str = "UnpopulatedParams";
const UNPOPULATED_STRUCT: u32 = 5;

/// How the probe is expected to answer for a Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expected {
    Holds,
    Absent,
    Fails,
}

fn raw(words: [u32; 2]) -> TestValue {
    TestValue::Raw(words.iter().flat_map(|word| word.to_le_bytes()).collect())
}

fn instance(type_name: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(type_name).value("size", TestValue::Int32(1)))
}

fn entity(index: u128, values: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        index,
        "Entity",
        format!("Entity.E{index}"),
        format!("e{index}.xml"),
    )
    .value("Components", TestValue::Array(values))
}

fn forge_bytes() -> Vec<u8> {
    forge_builder().build()
}

fn forge_builder() -> DataForgeBuilder {
    let base = |name: &str| {
        TestStruct::new(name)
            .parent("Base")
            .field("size", TestType::Int32)
    };
    let mut builder = DataForgeBuilder::new()
        .structure(TestStruct::new("Base"))
        .structure(base(COMPONENT))
        .structure(base(OTHER))
        .structure(base("CrafterDerivedParams").parent(COMPONENT))
        .structure(
            TestStruct::new("Entity")
                .array("Components", TestType::StrongPointer("Base".to_owned())),
        )
        .structure(base(UNPOPULATED));
    for record in cases().into_iter().map(|(record, _)| record) {
        builder = builder.record(record);
    }
    builder
}

fn cases() -> Vec<(TestRecord, Expected)> {
    vec![
        (
            entity(1, vec![instance("OtherParams"), instance(COMPONENT)]),
            Expected::Holds,
        ),
        (entity(2, vec![instance("OtherParams")]), Expected::Absent),
        (
            entity(3, vec![instance("CrafterDerivedParams")]),
            Expected::Absent,
        ),
        (entity(4, Vec::new()), Expected::Absent),
        (entity(5, vec![raw([u32::MAX, u32::MAX])]), Expected::Absent),
        (entity(6, vec![raw([99, 0])]), Expected::Absent),
        (entity(7, vec![raw([CRAFTER_STRUCT, 77])]), Expected::Absent),
        (
            entity(8, Vec::new()).value("Components", raw([1000, 0])),
            Expected::Fails,
        ),
        (
            entity(9, Vec::new()).value("Components", raw([u32::MAX, 5])),
            Expected::Fails,
        ),
        (
            entity(10, Vec::new()).value("Components", raw([u32::MAX, u32::MAX])),
            Expected::Fails,
        ),
        (
            entity(11, vec![instance(COMPONENT), raw([CRAFTER_STRUCT, 77])]),
            Expected::Holds,
        ),
        (
            entity(12, vec![instance(COMPONENT), raw([OTHER_STRUCT, 77])]),
            Expected::Holds,
        ),
        (entity(13, vec![raw([OTHER_STRUCT, 77])]), Expected::Absent),
        (
            entity(14, vec![instance(COMPONENT), raw([99, 0])]),
            Expected::Holds,
        ),
        (
            entity(15, vec![raw([UNPOPULATED_STRUCT, 0])]),
            Expected::Absent,
        ),
    ]
}

fn tree_holds(record: &Record<'_>) -> Result<bool, DataForgeError> {
    let tree = record.tree()?;
    Ok(matches!(tree.field("Components"), Some(Value::Array(items))
            if items.iter().any(|item| matches!(item, Value::Struct(value) if value.type_name() == COMPONENT))))
}

#[test]
fn the_probe_agrees_with_the_tree_and_fails_on_corrupt_arrays() -> TestResult {
    let bytes = forge_bytes();
    let forge = DataForge::open(&bytes)?;
    let probe = forge
        .pointer_probe("Components", COMPONENT)
        .ok_or("a struct with instances")?;
    let mut trees = TreeBuilder::new();
    let expected: Vec<_> = cases().into_iter().map(|(_, expected)| expected).collect();
    for (record, expected) in forge.records().zip(expected) {
        let got = trees.holds(&probe, &record);
        match (expected, got) {
            (Expected::Holds, Ok(true)) | (Expected::Absent, Ok(false)) => {
                assert_eq!(tree_holds(&record)?, expected == Expected::Holds, "tree");
            }
            (Expected::Fails, Err(_)) => {}
            (expected, got) => panic!("{}: {expected:?}, got {got:?}", record.name()?),
        }
    }
    Ok(())
}

#[test]
fn no_probe_is_made_for_a_struct_without_instances() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new(COMPONENT))
        .build();
    let forge = DataForge::open(&bytes)?;
    assert!(forge.pointer_probe("Components", COMPONENT).is_none());
    assert!(forge.pointer_probe("Components", "Missing").is_none());
    Ok(())
}
