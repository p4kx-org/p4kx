// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType, RecordTree, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::record_field_storage_source::TestResult;

pub(crate) const OWNER: u128 = 0x5a11;
pub(crate) const MISSING: u128 = 0x5a12;
pub(crate) const INVALID_STRUCT: u32 = 77;
pub(crate) const INVALID_INSTANCE: u32 = 99;

fn builder() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Base").field("inherited", TestType::UInt32))
        .structure(TestStruct::new("Node").field("label", TestType::UInt32))
}

pub(crate) fn duplicates() -> TestResult<Vec<u8>> {
    let data = builder()
        .structure(
            TestStruct::new("Owner")
                .parent("Base")
                .field("slotA", TestType::StrongPointer("Node".to_owned()))
                .field("slotB", TestType::StrongPointer("Node".to_owned()))
                .field("first", TestType::WeakPointer("Node".to_owned()))
                .field("second", TestType::WeakPointer("Node".to_owned())),
        )
        .instance(
            "left",
            TestInstance::new("Node").value("label", TestValue::UInt32(11)),
        )
        .instance(
            "right",
            TestInstance::new("Node").value("label", TestValue::UInt32(22)),
        )
        .record(
            TestRecord::new(OWNER, "Owner", "Owner.One", "owner.xml")
                .value("inherited", TestValue::UInt32(7))
                .value("slotA", TestValue::pointer("left"))
                .value("slotB", TestValue::pointer("right"))
                .value("first", TestValue::pointer("left"))
                .value("second", TestValue::pointer("right")),
        )
        .build();
    rename_slot(data)
}

fn rename_slot(mut data: Vec<u8>) -> TestResult<Vec<u8>> {
    const ORIGINAL: &[u8] = b"slotB\0";
    const REPLACEMENT: &[u8] = b"slotA\0";
    let positions: Vec<_> = data
        .windows(ORIGINAL.len())
        .enumerate()
        .filter_map(|(position, bytes)| (bytes == ORIGINAL).then_some(position))
        .collect();
    let [position] = positions.as_slice() else {
        return Err("one original synthetic field-name occurrence".into());
    };
    assert_eq!(
        ORIGINAL.len(),
        REPLACEMENT.len(),
        "all source offsets stay unchanged"
    );
    data.get_mut(*position..*position + ORIGINAL.len())
        .ok_or("original stored field-name bytes")?
        .copy_from_slice(REPLACEMENT);
    Ok(data)
}

pub(crate) fn nested() -> Vec<u8> {
    builder()
        .structure(
            TestStruct::new("Wrapper").field("inner", TestType::StrongPointer("Node".to_owned())),
        )
        .structure(nested_owner())
        .record(nested_record())
        .build()
}

fn nested_owner() -> TestStruct {
    TestStruct::new("Owner")
        .parent("Base")
        .array("nodes", TestType::Class("Node".to_owned()))
        .field("wrapper", TestType::Class("Wrapper".to_owned()))
        .array("links", TestType::WeakPointer("Node".to_owned()))
        .field("self", TestType::WeakPointer("Owner".to_owned()))
        .field("record", TestType::Reference)
        .field("empty", TestType::WeakPointer("Node".to_owned()))
        .field("invalid", TestType::WeakPointer("Node".to_owned()))
        .field("missing", TestType::Reference)
}

fn nested_record() -> TestRecord {
    let invalid = [INVALID_STRUCT.to_le_bytes(), INVALID_INSTANCE.to_le_bytes()].concat();
    TestRecord::new(OWNER, "Owner", "Owner.One", "owner.xml")
        .value("inherited", TestValue::UInt32(7))
        .value("nodes", nested_nodes())
        .value("wrapper", nested_wrapper())
        .value(
            "links",
            TestValue::Array(vec![
                TestValue::pointer("right"),
                TestValue::pointer("inner"),
                TestValue::pointer("right"),
                TestValue::Null,
            ]),
        )
        .value("self", TestValue::pointer("Owner.One"))
        .value("record", TestValue::Reference(OWNER))
        .value("empty", TestValue::Null)
        .value("invalid", TestValue::Raw(invalid))
        .value("missing", TestValue::Reference(MISSING))
}

fn nested_nodes() -> TestValue {
    TestValue::Array(vec![
        TestValue::Struct(
            TestInstance::new("Node")
                .named("left")
                .value("label", TestValue::UInt32(11)),
        ),
        TestValue::Struct(
            TestInstance::new("Node")
                .named("right")
                .value("label", TestValue::UInt32(22)),
        ),
    ])
}

fn nested_wrapper() -> TestValue {
    TestValue::Struct(
        TestInstance::new("Wrapper").value(
            "inner",
            TestValue::Struct(
                TestInstance::new("Node")
                    .named("inner")
                    .value("label", TestValue::UInt32(33)),
            ),
        ),
    )
}

pub(crate) fn original_duplicates<'source>(
    forge: &'source DataForge<'source>,
) -> TestResult<RecordTree<'source>> {
    let tree = original_tree(forge)?;
    let fields = tree.fields();
    let names: Vec<_> = fields.iter().map(p4kx_dataforge::Field::name).collect();
    assert_eq!(
        names,
        ["inherited", "slotA", "slotA", "first", "second"],
        "original declaration order retains both physical same-name fields"
    );
    for (index, label) in [(1, 11), (2, 22)] {
        let field = fields
            .get(index)
            .ok_or("original same-name field ordinal")?;
        assert_eq!(
            field.storage().data_type(),
            DataType::StrongPointer,
            "actual owned storage"
        );
        let child = structure(field.value())?;
        assert_eq!(
            child.type_name(),
            "Node",
            "the original concrete owned type"
        );
        assert_eq!(
            child.field("label"),
            Some(&Value::UInt32(label)),
            "distinct original owned payload"
        );
    }
    Ok(tree)
}

pub(crate) fn original_nested<'source>(
    forge: &'source DataForge<'source>,
) -> TestResult<RecordTree<'source>> {
    let tree = original_tree(forge)?;
    let owner = forge.record(0).ok_or("actual inherited owner")?;
    let parent = owner
        .struct_type()?
        .parent()
        .ok_or("actual inherited parent")?;
    assert_eq!(
        parent.name()?,
        "Base",
        "the original declaration actually inherits"
    );
    original_nested_layout(&tree)?;
    original_nested_values(&tree)?;
    Ok(tree)
}

fn original_nested_layout(tree: &RecordTree<'_>) -> TestResult {
    let fields = tree.fields();
    assert_eq!(
        fields.first().map(p4kx_dataforge::Field::name),
        Some("inherited"),
        "inherited field is physically first"
    );
    assert_eq!(
        tree.field("inherited"),
        Some(&Value::UInt32(7)),
        "actual inherited payload"
    );
    let node_field = fields.get(1).ok_or("original class-array field")?;
    assert_eq!(
        node_field.name(),
        "nodes",
        "physical field ordinal follows inheritance"
    );
    assert_eq!(
        node_field.storage().data_type(),
        DataType::Class,
        "actual class array storage"
    );
    assert!(
        node_field.storage().is_array(),
        "actual field is declared as an array"
    );
    Ok(())
}

fn original_nested_values(tree: &RecordTree<'_>) -> TestResult {
    let Some(Value::Array(nodes)) = tree.field("nodes") else {
        return Err("actual class array".into());
    };
    assert_eq!(nodes.len(), 2, "both original owned array elements exist");
    for (value, label) in nodes.iter().zip([11, 22]) {
        assert_eq!(
            structure(value)?.field("label"),
            Some(&Value::UInt32(label)),
            "stored original array order"
        );
    }
    let wrapper = structure(tree.field("wrapper").ok_or("original inline wrapper")?)?;
    let inner = structure(
        wrapper
            .field("inner")
            .ok_or("original nested strong slot")?,
    )?;
    assert_eq!(
        inner.field("label"),
        Some(&Value::UInt32(33)),
        "actual nested owned payload"
    );
    Ok(())
}

fn original_tree<'source>(forge: &'source DataForge<'source>) -> TestResult<RecordTree<'source>> {
    let owner = forge.record(0).ok_or("actual original owner")?;
    assert_eq!(
        owner.guid().to_string(),
        "00000000-0000-0000-0000-000000005a11",
        "actual original owner GUID"
    );
    assert_eq!(
        owner.stored_file_name()?,
        "owner.xml",
        "original raw filename"
    );
    assert_eq!(owner.type_name()?, "Owner", "actual original owner type");
    let tree = owner.tree()?;
    assert!(
        tree.is_from(&owner),
        "actual Record/tree ownership precedes receipt assertions"
    );
    Ok(tree)
}

fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    match value {
        Value::Struct(value) => Ok(value),
        _ => Err("actual original owned struct".into()),
    }
}
