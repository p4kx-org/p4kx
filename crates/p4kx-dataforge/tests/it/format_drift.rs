// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Small drifts of the format are read and reported instead of failing or reading wrong data.

use std::error::Error;

use p4kx_dataforge::{
    DataForge, DataForgeIssue, Field, MAX_DATAFORGE_ISSUES, RecordTree, Value, ValueError,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

const SAMPLE: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;

fn value<'t, 'f>(tree: &'t RecordTree<'f>, name: &str) -> Result<&'t Value<'f>, String> {
    tree.fields()
        .iter()
        .find(|field| field.name() == name)
        .map(Field::value)
        .ok_or_else(|| format!("no field {name}"))
}

fn flagged() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Ship")
                .field("speed", TestType::Int32)
                .conversion_flags(0x01)
                .array("crew", TestType::Int32)
                .conversion_flags(0x80)
                .field("after", TestType::Int8),
        )
        .values(p4kx_testkit::TestPool::Int32, 2)
        .record(
            TestRecord::new(SAMPLE, "Ship", "Ship.One", "ship.xml")
                .value("speed", TestValue::Int32(7))
                .value("after", TestValue::Int8(5)),
        )
        .build()
}

#[test]
fn conversion_flags_above_the_low_byte_are_ignored_and_reported_once_per_property() -> TestResult {
    let bytes = flagged();
    let forge = DataForge::open(&bytes)?;

    let tree = forge.records().next().ok_or("no Record")?.tree()?;

    assert_eq!(value(&tree, "speed")?, &Value::Int32(7), "a single value");
    assert!(
        matches!(value(&tree, "crew")?, Value::Array(items) if items.is_empty()),
        "an array"
    );
    assert_eq!(value(&tree, "after")?, &Value::Int8(5), "the layout holds");
    assert_eq!(
        forge.issues(),
        [
            DataForgeIssue::ConversionFlags {
                property: 0,
                flags: 0x01
            },
            DataForgeIssue::ConversionFlags {
                property: 1,
                flags: 0x80
            },
        ],
        "one issue per property"
    );
    assert_eq!(forge.issue_count(), 2);
    Ok(())
}

#[test]
fn a_database_without_drift_has_no_issues() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .record(TestRecord::new(SAMPLE, "Ship", "Ship.One", "ship.xml").tag("Design"))
        .build();

    let forge = DataForge::open(&bytes)?;

    assert_eq!(forge.issues(), []);
    assert_eq!(forge.issue_count(), 0);
    let record = forge.records().next().ok_or("no Record")?;
    assert_eq!(record.name()?, "Ship.One");
    assert_eq!(record.tag()?, Some("Design"));
    Ok(())
}

#[test]
fn a_record_without_name_has_an_empty_name_and_an_issue() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .record(TestRecord::new(SAMPLE, "Ship", "Ship.One", "ship.xml").without_name())
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("no Record")?;

    assert_eq!(record.name()?, "");
    let tree = record.tree()?;
    assert_eq!(tree.name(), "");
    assert_eq!(tree.record_path(), "ship");
    assert_eq!(
        forge.issues(),
        [DataForgeIssue::RecordWithoutName { record: 0 }]
    );
    Ok(())
}

#[test]
fn a_record_without_tag_has_no_tag_and_an_issue() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .record(TestRecord::new(SAMPLE, "Ship", "Ship.One", "ship.xml").without_tag())
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("no Record")?;

    assert_eq!(record.tag()?, None);
    assert_eq!(record.name()?, "Ship.One");
    record.tree()?;
    assert_eq!(
        forge.issues(),
        [DataForgeIssue::RecordWithoutTag { record: 0 }]
    );
    Ok(())
}

#[test]
fn a_nameless_record_does_not_keep_its_siblings_from_building() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .record(TestRecord::new(SAMPLE, "Ship", "Ship.One", "one.xml").without_name())
        .record(TestRecord::new(SAMPLE + 1, "Ship", "Ship.Two", "two.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;

    let names: Result<Vec<_>, _> = forge.records().map(|record| record.name()).collect();

    assert_eq!(names?, ["", "Ship.Two"]);
    Ok(())
}

#[test]
fn the_issue_list_is_capped_but_counts_every_issue() -> TestResult {
    let total = MAX_DATAFORGE_ISSUES + 30;
    let bytes = (0..total)
        .fold(
            DataForgeBuilder::new().structure(TestStruct::new("Ship")),
            |builder, index| {
                builder.record(
                    TestRecord::new(SAMPLE + index as u128, "Ship", "Ship.One", "ship.xml")
                        .without_name(),
                )
            },
        )
        .build();

    let forge = DataForge::open(&bytes)?;

    assert_eq!(forge.issues().len(), MAX_DATAFORGE_ISSUES);
    assert_eq!(forge.issue_count(), total);
    Ok(())
}

#[test]
fn pointer_indices_are_read_as_32_bits() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Node"))
        .structure(
            TestStruct::new("Holder")
                .field("far", TestType::WeakPointer("Node".to_owned()))
                .field("near", TestType::WeakPointer("Node".to_owned()))
                .field("none", TestType::StrongPointer("Node".to_owned())),
        )
        .record(
            TestRecord::new(SAMPLE, "Holder", "Holder.One", "holder.xml")
                .value("far", raw([0, 0x1_0000]))
                .value("near", raw([0, 0])),
        )
        .record(TestRecord::new(SAMPLE + 1, "Node", "Node.One", "node.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;

    let tree = forge.records().next().ok_or("no Record")?.tree()?;

    assert_eq!(
        value(&tree, "far")?,
        &Value::Error(ValueError::InvalidPointer {
            struct_index: 0,
            instance_index: 0x1_0000
        }),
        "an index above 16 bits is not cut to 0"
    );
    let survey = forge.survey()?;
    assert_eq!(survey.pointers.dangling, 1);
    assert_eq!(survey.pointers.narrow, 1);
    assert_eq!(survey.pointers.null, 1);
    assert_eq!(survey.pointers.wide, 0);
    Ok(())
}

fn raw(words: [u32; 2]) -> TestValue {
    TestValue::Raw(words.iter().flat_map(|word| word.to_le_bytes()).collect())
}

#[test]
fn the_survey_reports_the_format_details_of_a_clean_database() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .record(TestRecord::new(SAMPLE, "Ship", "Ship.One", "one.xml").tag("Design"))
        .record(TestRecord::new(SAMPLE + 1, "Ship", "Ship.Two", "two.xml").without_tag())
        .build();
    let forge = DataForge::open(&bytes)?;

    let survey = forge.survey()?;

    assert_eq!(survey.header_words, [0, 0]);
    assert_eq!(survey.trailing.equals_struct_size, 2);
    assert_eq!(survey.trailing.other, 0);
    assert_eq!(survey.records_without_tag, 1);
    assert_eq!(survey.records_without_name, 0);
    assert_eq!(survey.flagged_properties, 0);
    assert_eq!(survey.unreadable_structs, 0);
    Ok(())
}

#[test]
fn the_survey_sees_header_words_and_odd_trailing_values() -> TestResult {
    let mut bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Ship").field("speed", TestType::Int32))
        .record(TestRecord::new(SAMPLE, "Ship", "Ship.One", "one.xml"))
        .build();
    let table = record_table_end(&bytes)?;
    patch(&mut bytes, 8, &5_u32.to_le_bytes())?;
    patch(&mut bytes, 12, &6_u32.to_le_bytes())?;
    patch(&mut bytes, table - 2, &9_u16.to_le_bytes())?;
    let forge = DataForge::open(&bytes)?;

    let survey = forge.survey()?;

    assert_eq!(survey.header_words, [5, 6]);
    assert_eq!(survey.trailing.other, 1);
    assert_eq!(survey.trailing.first_other, Some(9));
    Ok(())
}

fn patch(bytes: &mut [u8], at: usize, new: &[u8]) -> TestResult {
    bytes
        .get_mut(at..at + new.len())
        .ok_or("patch outside the database")?
        .copy_from_slice(new);
    Ok(())
}

fn record_table_end(bytes: &[u8]) -> Result<usize, Box<dyn Error>> {
    const HEADER: usize = 120;
    let word = |at: usize| -> Result<usize, Box<dyn Error>> {
        let field = bytes.get(at..at + 4).ok_or("short header")?;
        Ok(u32::from_le_bytes(field.try_into()?) as usize)
    };
    let tables = [(16, 16), (20, 12), (24, 8), (28, 8), (32, 32)];
    tables
        .iter()
        .try_fold(HEADER, |end, (at, row)| Ok(end + word(*at)? * row))
}
