// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The schema drift between two synthetic DataForges, through the public API.

use std::error::Error;

use p4kx_comparison::{
    DataForge, OptionChangeKind, PropertyAspect, SchemaChangeKind, SchemaDiff, SideCounts,
    compare_schema, schema_report,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType};

use crate::forge_pair::ForgePair;

type TestResult = Result<(), Box<dyn Error>>;

fn record(guid: u128, type_name: &str, name: &str) -> TestRecord {
    TestRecord::new(guid, type_name, name, format!("libs/{name}.xml"))
}

fn old_forge() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Medium", "Large"])
        .enumeration("Gone", ["A"])
        .structure(TestStruct::new("Base").field("name", TestType::String))
        .structure(
            TestStruct::new("Item")
                .parent("Base")
                .field("weight", TestType::Single)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("owner", TestType::Reference)
                .array("tags", TestType::String)
                .field("mass", TestType::Int32),
        )
        .structure(TestStruct::new("Part").field("value", TestType::Int32))
        .structure(TestStruct::new("Retired").field("flag", TestType::Bool))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0001,
            "Item",
            "a",
        ))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0002,
            "Item",
            "b",
        ))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0003,
            "Part",
            "c",
        ))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0004,
            "Retired",
            "d",
        ))
}

fn new_forge() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .enumeration("Size", ["Small", "Large", "Huge"])
        .enumeration("Fresh", ["Z"])
        .structure(
            TestStruct::new("Base")
                .field("name", TestType::String)
                .field("id", TestType::Guid),
        )
        .structure(
            TestStruct::new("Item")
                .parent("Base")
                .field("weight", TestType::Double)
                .field("size", TestType::Enum("Size".to_owned()))
                .field("owner", TestType::ReferenceTo("Part".to_owned()))
                .field("tags", TestType::String)
                .field("extra", TestType::Bool),
        )
        .structure(
            TestStruct::new("Part")
                .parent("Base")
                .field("value", TestType::Int32),
        )
        .structure(TestStruct::new("Fresh").field("flag", TestType::Bool))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0001,
            "Item",
            "a",
        ))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0002,
            "Item",
            "b",
        ))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0005,
            "Item",
            "e",
        ))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0003,
            "Part",
            "c",
        ))
        .record(record(
            0x1000_0000_0000_4000_8000_0000_0000_0006,
            "Fresh",
            "f",
        ))
}

fn diff(old: &DataForgeBuilder, new: &DataForgeBuilder) -> Result<SchemaDiff, Box<dyn Error>> {
    let (old, new) = (old.build(), new.build());
    Ok(compare_schema(
        &DataForge::open(&old)?,
        &DataForge::open(&new)?,
    ))
}

fn struct_change<'d>(
    diff: &'d SchemaDiff,
    name: &str,
) -> Result<&'d p4kx_comparison::StructChange, Box<dyn Error>> {
    diff.structs
        .iter()
        .find(|change| change.name == name)
        .ok_or_else(|| format!("no change of struct {name}").into())
}

#[test]
fn equal_schemas_do_not_differ() -> TestResult {
    let diff = diff(&old_forge(), &old_forge())?;

    assert!(!diff.has_differences(), "no difference: {diff:?}");
    assert_eq!(diff.counts.structs.total() + diff.counts.enums.total(), 0);
    Ok(())
}

#[test]
fn structs_are_added_removed_and_changed_with_their_records() -> TestResult {
    let diff = diff(&old_forge(), &new_forge())?;

    let names: Vec<(&str, SchemaChangeKind)> = diff
        .structs
        .iter()
        .map(|change| (change.name.as_str(), change.change))
        .collect();
    assert_eq!(
        names,
        [
            ("Base", SchemaChangeKind::Changed),
            ("Fresh", SchemaChangeKind::Added),
            ("Item", SchemaChangeKind::Changed),
            ("Part", SchemaChangeKind::Changed),
            ("Retired", SchemaChangeKind::Removed),
        ]
    );
    assert_eq!(
        struct_change(&diff, "Fresh")?.records,
        SideCounts { old: 0, new: 1 }
    );
    assert_eq!(
        struct_change(&diff, "Retired")?.records,
        SideCounts { old: 1, new: 0 }
    );
    assert_eq!(
        struct_change(&diff, "Item")?.records,
        SideCounts { old: 2, new: 3 }
    );
    assert_eq!(
        struct_change(&diff, "Base")?.records,
        SideCounts { old: 2, new: 4 },
        "a struct counts the Records of the structs inheriting from it"
    );
    assert_eq!(diff.counts.structs.added, 1);
    assert_eq!(diff.counts.structs.removed, 1);
    assert_eq!(diff.counts.structs.changed, 3);
    Ok(())
}

#[test]
fn a_changed_parent_is_reported() -> TestResult {
    let diff = diff(&old_forge(), &new_forge())?;

    let part = struct_change(&diff, "Part")?;

    let parent = part.parent.as_ref().ok_or("parent change")?;
    assert_eq!(parent.old, None);
    assert_eq!(parent.new.as_deref(), Some("Base"));
    assert!(part.properties.is_empty(), "no property differs");
    Ok(())
}

#[test]
fn properties_differ_in_type_conversion_and_target() -> TestResult {
    let diff = diff(&old_forge(), &new_forge())?;

    let item = struct_change(&diff, "Item")?;

    let changes: Vec<(&str, SchemaChangeKind, Vec<PropertyAspect>)> = item
        .properties
        .iter()
        .map(|change| (change.name.as_str(), change.change, change.aspects.clone()))
        .collect();
    assert_eq!(
        changes,
        [
            ("extra", SchemaChangeKind::Added, vec![]),
            ("mass", SchemaChangeKind::Removed, vec![]),
            (
                "owner",
                SchemaChangeKind::Changed,
                vec![PropertyAspect::Target]
            ),
            (
                "tags",
                SchemaChangeKind::Changed,
                vec![PropertyAspect::Conversion]
            ),
            (
                "weight",
                SchemaChangeKind::Changed,
                vec![PropertyAspect::DataType]
            ),
        ]
    );
    let weight = item
        .properties
        .iter()
        .find(|change| change.name == "weight")
        .ok_or("weight")?;
    assert_eq!(
        weight.old.as_ref().map(|shape| shape.data_type.as_str()),
        Some("Single")
    );
    assert_eq!(
        weight.new.as_ref().map(|shape| shape.data_type.as_str()),
        Some("Double")
    );
    let owner = item
        .properties
        .iter()
        .find(|change| change.name == "owner")
        .ok_or("owner")?;
    assert_eq!(
        owner.old.as_ref().and_then(|shape| shape.target.as_deref()),
        None
    );
    assert_eq!(
        owner.new.as_ref().and_then(|shape| shape.target.as_deref()),
        Some("Part")
    );
    assert_eq!(diff.counts.properties.added, 3, "includes Fresh.flag");
    assert_eq!(diff.counts.properties.removed, 2, "includes Retired.flag");
    assert_eq!(diff.counts.properties.changed, 3, "shared properties");
    Ok(())
}

#[test]
fn enums_and_options_are_added_removed_and_changed() -> TestResult {
    let diff = diff(&old_forge(), &new_forge())?;

    let enums: Vec<(&str, SchemaChangeKind)> = diff
        .enums
        .iter()
        .map(|change| (change.name.as_str(), change.change))
        .collect();
    assert_eq!(
        enums,
        [
            ("Fresh", SchemaChangeKind::Added),
            ("Gone", SchemaChangeKind::Removed),
            ("Size", SchemaChangeKind::Changed),
        ]
    );
    let size = diff
        .enums
        .iter()
        .find(|change| change.name == "Size")
        .ok_or("Size")?;
    let options: Vec<(&str, OptionChangeKind)> = size
        .options
        .iter()
        .map(|option| (option.name.as_str(), option.change))
        .collect();
    assert_eq!(
        options,
        [
            ("Huge", OptionChangeKind::Added),
            ("Medium", OptionChangeKind::Removed)
        ]
    );
    assert_eq!(
        size.records,
        SideCounts { old: 2, new: 3 },
        "the Records of the structs that declare a property of the enum"
    );
    assert_eq!(diff.counts.options.added, 1);
    assert_eq!(diff.counts.options.removed, 1);
    Ok(())
}

#[test]
fn the_report_names_both_sides_and_carries_the_diff() -> TestResult {
    let pair = ForgePair::new(&old_forge(), &new_forge())?;
    let (old, new) = (pair.old_entry.open()?, pair.new_entry.open()?);
    let diff = compare_schema(&old, &new);

    let report = serde_json::to_value(schema_report(&pair.old, &pair.new, &diff, None))?;

    assert_eq!(report.pointer("/format_version"), Some(&1.into()));
    assert_eq!(report.pointer("/source/old/channel"), Some(&"LIVE".into()));
    assert_eq!(report.pointer("/source/new/channel"), Some(&"PTU".into()));
    assert_eq!(report.pointer("/counts/structs/added"), Some(&1.into()));
    assert_eq!(report.pointer("/structs/0/name"), Some(&"Base".into()));
    assert_eq!(report.pointer("/structs/0/change"), Some(&"changed".into()));
    assert_eq!(report.pointer("/structs/0/records/new"), Some(&4.into()));
    assert_eq!(
        report.pointer("/structs/0/mapped_instances/new"),
        Some(&0.into()),
        "the base struct has no direct mappings despite its inherited Record count"
    );
    assert_eq!(
        report.pointer("/enums/2/options/0/name"),
        Some(&"Huge".into())
    );
    Ok(())
}
