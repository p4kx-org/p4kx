// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Record counts and the edge cases of the schema drift.

use std::error::Error;

use p4kx_comparison::{DataForge, SchemaChangeKind, SchemaDiff, compare_schema};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn diff(old: &DataForgeBuilder, new: &DataForgeBuilder) -> Result<SchemaDiff, Box<dyn Error>> {
    let (old, new) = (old.build(), new.build());
    Ok(compare_schema(
        &DataForge::open(&old)?,
        &DataForge::open(&new)?,
    ))
}

fn record(guid: u128, type_name: &str) -> TestRecord {
    TestRecord::new(guid, type_name, "r", format!("libs/{guid}.xml"))
}

fn wheels(old: bool) -> DataForgeBuilder {
    let wheel =
        |name: &str| TestValue::Struct(p4kx_testkit::TestInstance::new("Wheel").named(name));
    let car = TestStruct::new("Car").array("wheels", TestType::StrongPointer("Wheel".to_owned()));
    let wheel_struct = if old {
        TestStruct::new("Wheel").field("grip", TestType::Int32)
    } else {
        TestStruct::new("Wheel").field("grip", TestType::Single)
    };
    DataForgeBuilder::new()
        .structure(wheel_struct)
        .structure(car)
        .record(
            record(0x1000_0000_0000_4000_8000_0000_0000_0001, "Car").value(
                "wheels",
                TestValue::Array(vec![wheel("a"), wheel("b"), wheel("c")]),
            ),
        )
}

#[test]
fn a_struct_that_only_strong_pointers_hold_has_no_records() -> TestResult {
    let diff = diff(&wheels(true), &wheels(false))?;

    let wheel = diff
        .structs
        .iter()
        .find(|change| change.name == "Wheel")
        .ok_or("Wheel")?;
    assert_eq!(wheel.change, SchemaChangeKind::Changed);
    assert_eq!(
        (wheel.records.old, wheel.records.new),
        (0, 0),
        "three instances of Wheel, but none is a Record"
    );
    Ok(())
}

fn inherited(extra: bool) -> DataForgeBuilder {
    let base = TestStruct::new("Base").field("size", TestType::Enum("Size".to_owned()));
    let child = TestStruct::new("Child").parent("Base");
    DataForgeBuilder::new()
        .enumeration("Size", if extra { vec!["A", "B"] } else { vec!["A"] })
        .structure(base)
        .structure(child)
        .record(record(0x2000_0000_0000_4000_8000_0000_0000_0001, "Base"))
        .record(record(0x2000_0000_0000_4000_8000_0000_0000_0002, "Child"))
        .record(record(0x2000_0000_0000_4000_8000_0000_0000_0003, "Child"))
}

#[test]
fn an_enum_counts_each_record_once_through_a_parent_and_its_child() -> TestResult {
    let diff = diff(&inherited(false), &inherited(true))?;

    let size = diff
        .enums
        .iter()
        .find(|change| change.name == "Size")
        .ok_or("Size")?;
    assert_eq!((size.records.old, size.records.new), (3, 3));
    Ok(())
}

#[test]
fn a_struct_that_is_its_own_parent_does_not_hang_or_overflow() -> TestResult {
    let looping = |size: u32| {
        (0..size).fold(
            DataForgeBuilder::new().structure(TestStruct::new("Loop").parent("Loop")),
            |builder, index| {
                builder.structure(TestStruct::new(format!("Child{index}")).parent("Loop"))
            },
        )
    };

    let diff = diff(&looping(50), &looping(60))?;

    assert_eq!(diff.counts.structs.added, 10);
    Ok(())
}

#[test]
fn a_name_defined_twice_counts_as_left_out_and_is_a_difference() -> TestResult {
    let single =
        DataForgeBuilder::new().structure(TestStruct::new("Dup").field("a", TestType::Bool));
    let twice = single
        .clone()
        .structure(TestStruct::new("Dup").field("b", TestType::Bool));

    let diff = diff(&single, &twice)?;

    assert_eq!((diff.unreadable.old, diff.unreadable.new), (0, 1));
    assert!(diff.structs.is_empty(), "the first definition wins");
    assert!(diff.has_differences(), "a different number was left out");
    Ok(())
}

#[path = "schema_counts_inheritance.rs"]
mod schema_counts_inheritance;
