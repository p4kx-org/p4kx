// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! One `TreeBuilder` for many Records, also of another DataForge.

use std::error::Error;

use p4kx_dataforge::{DataForge, TreeBuilder, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

const FIRST: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const SECOND: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;

fn database(type_name: &str, field: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new(type_name).field(field, TestType::Int32))
        .record(
            TestRecord::new(FIRST, type_name, "One", "one.xml").value(field, TestValue::Int32(1)),
        )
        .record(
            TestRecord::new(SECOND, type_name, "Two", "two.xml").value(field, TestValue::Int32(2)),
        )
        .build()
}

#[test]
fn a_builder_gives_the_same_trees_as_building_each_record_alone() -> TestResult {
    let ships = database("Ship", "crew");
    let items = database("Item", "mass");
    let ships = DataForge::open(&ships)?;
    let items = DataForge::open(&items)?;
    let mut builder = TreeBuilder::new();

    for forge in [&ships, &items, &ships] {
        for record in forge.records() {
            assert_eq!(builder.build(&record)?, record.tree()?, "{}", record.guid());
        }
    }
    Ok(())
}

#[test]
fn a_record_knows_its_dataforge() -> TestResult {
    let ships = database("Ship", "crew");
    let items = database("Item", "mass");
    let ships = DataForge::open(&ships)?;
    let items = DataForge::open(&items)?;

    let record = ships.records().next().ok_or("no Record")?;

    assert!(record.is_from(&ships), "its own DataForge");
    assert!(!record.is_from(&items), "another DataForge");
    Ok(())
}

#[test]
fn the_root_is_walked_like_a_nested_struct() -> TestResult {
    let ships = database("Ship", "crew");
    let ships = DataForge::open(&ships)?;

    let tree = ships.records().next().ok_or("no Record")?.tree()?;

    assert_eq!(tree.root().type_name(), "Ship", "the type");
    assert_eq!(tree.root().fields(), tree.fields(), "the same fields");
    assert_eq!(tree.field("crew"), Some(&Value::Int32(1)), "by name");
    assert_eq!(
        tree.root().field("crew"),
        Some(&Value::Int32(1)),
        "on the root"
    );
    assert_eq!(tree.field("missing"), None, "no such field");
    Ok(())
}

#[test]
fn one_value_keeps_its_existing_compact_layout() {
    let value = size_of::<Value<'_>>();

    assert!(value <= 48, "one value takes {value} bytes");
}
