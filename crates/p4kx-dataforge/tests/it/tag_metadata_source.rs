// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataForgeError, SNIFF_LEN, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const STRUCT_ROW_BYTES: usize = 16;
const UNKNOWN_STRUCT: usize = 2;

pub(crate) struct Source {
    pub(crate) bytes: Vec<u8>,
    pub(crate) other: u32,
}

pub(crate) fn source(taxonomy: bool, malformed: bool) -> TestResult<Source> {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .array("children", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(TestStruct::new("Other").field("value", TestType::Int32));
    if taxonomy {
        builder = builder
            .record(
                TestRecord::new(1, "Tag", "Tag.Root", "source/tag.xml")
                    .value("tagName", TestValue::text("Root"))
                    .value("children", TestValue::Array(Vec::new())),
            )
            .record(
                TestRecord::new(2, "TagDatabase", "TagDatabase.Roots", "source/roots.xml")
                    .value("tags", TestValue::Array(vec![TestValue::Reference(1)])),
            );
    }
    let mut bytes = builder
        .record(
            TestRecord::new(3, "Other", "Other.Source", "source/other.xml")
                .value("value", TestValue::Int32(7)),
        )
        .build();
    if malformed {
        let at = SNIFF_LEN + UNKNOWN_STRUCT * STRUCT_ROW_BYTES;
        bytes
            .get_mut(at..at + 4)
            .ok_or("the actual Struct.name word")?
            .copy_from_slice(&u32::MAX.to_le_bytes());
    }
    Ok(Source {
        bytes,
        other: if taxonomy { 2 } else { 0 },
    })
}

pub(crate) fn unknown_original(
    forge: &DataForge<'_>,
    source: &Source,
) -> TestResult<DataForgeError> {
    let at = SNIFF_LEN + UNKNOWN_STRUCT * STRUCT_ROW_BYTES;
    assert_eq!(
        source
            .bytes
            .get(at..at + 4)
            .ok_or("the original Struct.name bytes")?,
        u32::MAX.to_le_bytes(),
        "only the actual type-name selector was made invalid"
    );
    let record = forge
        .record(source.other)
        .ok_or("the actual unclassified source Record")?;
    assert_eq!(
        record.struct_type()?.index(),
        u32::try_from(UNKNOWN_STRUCT)?,
        "the actual Record selects that Struct row"
    );
    let error = record
        .type_name()
        .err()
        .ok_or("the actual Struct selector cannot name a family")?;
    assert_eq!(
        error,
        DataForgeError::InvalidString {
            table: "blob",
            offset: u32::MAX
        },
        "Struct names have no no-string sentinel"
    );
    Ok(error)
}

pub(crate) fn other_original(forge: &DataForge<'_>, source: &Source) -> TestResult {
    let record = forge
        .record(source.other)
        .ok_or("the actual non-Tag source")?;
    assert_eq!(
        record.type_name()?,
        "Other",
        "the family is genuinely known and unrelated"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the source view belongs to this physical Record"
    );
    assert_eq!(
        tree.field("value"),
        Some(&Value::Int32(7)),
        "the actual unrelated payload is readable"
    );
    Ok(())
}
