// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::layout_field_raw::{self as raw, Tables};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(super) fn array_bytes(count: u32) -> TestResult<Vec<u8>> {
    let mut bytes = crate::layout_field_array_source::database()?;
    let tables = Tables::read(&bytes)?;
    raw::write_word(&mut bytes, tables.mappings, count)?;
    raw::write_word(&mut bytes, tables.instances, count)?;
    Ok(bytes)
}

pub(super) fn original_array(bytes: &[u8], count: u32) -> TestResult {
    let tables = Tables::read(bytes)?;
    assert_eq!(
        raw::word(bytes, tables.mappings)?,
        count,
        "actual zero-size instance count"
    );
    assert_eq!(
        raw::word(bytes, tables.instances)?,
        count,
        "actual selected Array count"
    );
    assert_eq!(
        raw::word(bytes, tables.instances + 4)?,
        0,
        "actual first instance"
    );
    assert_eq!(
        bytes.len() - tables.instances,
        8,
        "only the real Array descriptor is stored"
    );
    let forge = DataForge::open(bytes)?;
    let empty = forge.structs().next().ok_or("actual Empty definition")?;
    assert_eq!(
        (empty.size(), empty.instance_count()),
        (0, count),
        "actual Empty mapping"
    );
    assert_eq!(
        empty.properties().count(),
        0,
        "no hidden original Empty fields"
    );
    let record = forge.record(0).ok_or("actual Holder Record")?;
    let holder = record.struct_type()?;
    assert_eq!(
        (holder.index(), holder.size()),
        (1, 8),
        "actual selected owner"
    );
    let field = holder
        .properties()
        .next()
        .ok_or("actual Array declaration")?;
    assert_eq!(field.name()?, "entries", "actual stored field name");
    assert_eq!(
        field.data_type()?,
        DataType::Class,
        "actual Class element storage"
    );
    assert!(field.is_array(), "actual Array declaration");
    assert_eq!(
        field.target_struct().ok_or("actual target")?.index(),
        0,
        "actual Empty target"
    );
    Ok(())
}

pub(super) fn malformed_bool() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Source").field("flag", TestType::Bool))
        .record(
            TestRecord::new(0xB001, "Source", "Source.Bool", "source/bool.xml")
                .value("flag", TestValue::Raw(vec![2])),
        )
        .build()
}

pub(super) fn empty() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Empty"))
        .record(TestRecord::new(
            0xB000,
            "Empty",
            "Empty.Zero",
            "source/empty.xml",
        ))
        .build()
}

pub(super) fn nested_owned() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Root")
                .field("owned", TestType::StrongPointer("Leaf".to_owned()))
                .array("values", TestType::UInt8),
        )
        .structure(
            TestStruct::new("Leaf")
                .field("left", TestType::Bool)
                .field("right", TestType::Int32),
        )
        .record(
            TestRecord::new(0xB007, "Root", "Root.Owned", "source/owned.xml")
                .value(
                    "owned",
                    TestValue::Struct(
                        TestInstance::new("Leaf")
                            .value("left", TestValue::Bool(true))
                            .value("right", TestValue::Int32(-7)),
                    ),
                )
                .value(
                    "values",
                    TestValue::Array(vec![
                        TestValue::UInt8(3),
                        TestValue::UInt8(0),
                        TestValue::UInt8(9),
                    ]),
                ),
        )
        .build()
}
