// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_comparison::{DataForge, SchemaDiff};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType};

type TestResult = Result<(), Box<dyn Error>>;

fn database(extra: bool) -> Vec<u8> {
    let mut root = TestStruct::new("N000").field("size", TestType::Enum("Size".to_owned()));
    if extra {
        root = root.field("mark", TestType::Int8);
    }
    let mut builder = DataForgeBuilder::new()
        .enumeration("Size", if extra { vec!["A", "B"] } else { vec!["A"] })
        .structure(root);
    for index in 1..70 {
        builder = builder.structure(
            TestStruct::new(format!("N{index:03}")).parent(format!("N{:03}", index - 1)),
        );
    }
    builder
        .record(TestRecord::new(1, "N069", "Example", "example.xml"))
        .build()
}

fn diff() -> Result<SchemaDiff, Box<dyn Error>> {
    let old_bytes = database(false);
    let new_bytes = database(true);
    let old = DataForge::open(&old_bytes)?;
    let new = DataForge::open(&new_bytes)?;
    for forge in [&old, &new] {
        assert_eq!(forge.record_count(), 1, "actual stored descendant");
        let record = forge.records().next().ok_or("Record")?;
        assert_eq!(record.struct_type()?.name()?, "N069", "actual leaf type");
        assert_eq!(record.tree()?.error_count(), 0, "valid original layout");
    }
    Ok(p4kx_comparison::compare_schema(&old, &new))
}

#[test]
fn distant_parent_change_counts_the_original_descendant() -> TestResult {
    let diff = diff()?;
    assert_eq!(
        (diff.unreadable.old, diff.unreadable.new),
        (0, 0),
        "valid original definitions"
    );
    let root = diff
        .structs
        .iter()
        .find(|item| item.name == "N000")
        .ok_or("root change")?;
    assert_eq!(
        (root.records.old, root.records.new),
        (1, 1),
        "the root counts its actual descendant"
    );
    Ok(())
}

#[test]
fn distant_inherited_enum_counts_the_original_descendant_once() -> TestResult {
    let diff = diff()?;
    let enumeration = diff
        .enums
        .iter()
        .find(|item| item.name == "Size")
        .ok_or("enum change")?;
    assert_eq!(
        (enumeration.records.old, enumeration.records.new),
        (1, 1),
        "the inherited enum counts the actual Record once"
    );
    Ok(())
}
