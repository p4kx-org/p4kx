// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::layout_field_fixture::TestResult;
use crate::layout_field_raw::{self as raw, Tables};

#[test]
fn tiny_original_metadata_can_name_a_full_u32_zero_size_class_array() -> TestResult {
    let bytes = database()?;
    let tables = Tables::read(&bytes)?;
    assert_eq!(
        (raw::word(&bytes, 16)?, raw::word(&bytes, 20)?),
        (2, 1),
        "the source has two actual definitions and one property"
    );
    assert_eq!(
        raw::word(&bytes, tables.mappings)?,
        u32::MAX,
        "the original instance_count is a full u32 even when each instance has zero bytes"
    );
    let payload = bytes
        .get(tables.instances..)
        .ok_or("the original holder instance")?;
    assert_eq!(
        payload.len(),
        8,
        "only one actual array descriptor occupies payload bytes"
    );
    assert_eq!(
        (raw::word(payload, 0)?, raw::word(payload, 4)?),
        (u32::MAX, 0),
        "the actual count and first element fit the original two u32 words"
    );
    let forge = DataForge::open(&bytes)?;
    schema(&forge)?;
    assert_eq!(
        forge
            .record(0)
            .ok_or("the original array owner")?
            .struct_type()?
            .index(),
        1,
        "the actual Record selects the eight-byte Holder"
    );
    Ok(())
}

pub(crate) fn schema(forge: &DataForge<'_>) -> TestResult {
    let empty = forge
        .structs()
        .next()
        .ok_or("the original Empty definition")?;
    assert_eq!(
        (empty.index(), empty.size(), empty.instance_count()),
        (0, 0, u32::MAX),
        "all named Empty instances are representable without instance payload bytes"
    );
    assert_eq!(
        empty.properties().count(),
        0,
        "the actual Empty class has no hidden members"
    );
    let holder = forge
        .record(0)
        .ok_or("the original Holder")?
        .struct_type()?;
    assert_eq!(
        (holder.size(), holder.instance_count()),
        (8, 1),
        "the actual mapped Holder width"
    );
    let property = holder
        .properties()
        .next()
        .ok_or("the original Class array declaration")?;
    assert_eq!(property.name()?, "entries", "the actual original field");
    assert_eq!(
        property.data_type()?,
        DataType::Class,
        "the real element storage"
    );
    assert!(property.is_array(), "the actual declaration is an array");
    assert_eq!(
        property
            .target_struct()
            .ok_or("the original Class target")?
            .index(),
        0,
        "the target word selects the actual zero-size Empty definition"
    );
    Ok(())
}

pub(crate) fn database() -> TestResult<Vec<u8>> {
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Empty").declared_size(0))
        .structure(TestStruct::new("Holder").array("entries", TestType::Class("Empty".to_owned())))
        .record(
            TestRecord::new(0x9999, "Holder", "Holder.Source", "source/zero-array.xml")
                .value("entries", TestValue::Array(Vec::new())),
        )
        .build();
    let tables = Tables::read(&bytes)?;
    raw::write_word(&mut bytes, tables.mappings, u32::MAX)?;
    raw::write_word(&mut bytes, tables.instances, u32::MAX)?;
    Ok(bytes)
}
