// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType, RecordTree, Reference, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::record_field_storage_source::TestResult;

pub(crate) fn reference<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree Reference<'source>> {
    match value {
        Value::Reference(reference) => Ok(reference),
        _ => Err("genuine original resolved Reference".into()),
    }
}

pub(crate) fn values<'tree, 'source>(
    tree: &'tree RecordTree<'source>,
    array: bool,
) -> TestResult<&'tree [Value<'source>]> {
    let value = tree.field("Link").ok_or("original link field")?;
    if array {
        let Value::Array(values) = value else {
            return Err("actual original array".into());
        };
        Ok(values.as_ref())
    } else {
        Ok(std::slice::from_ref(value))
    }
}

pub(crate) fn nested() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Node").field("label", TestType::String))
        .structure(
            TestStruct::new("Owner")
                .field("child", TestType::StrongPointer("Node".to_owned()))
                .field("peer", TestType::WeakPointer("Node".to_owned()))
                .field("self", TestType::WeakPointer("Owner".to_owned())),
        )
        .record(
            TestRecord::new(0x5911, "Owner", "Owner.One", "owner.xml")
                .value(
                    "child",
                    TestValue::Struct(
                        TestInstance::new("Node")
                            .named("owned")
                            .value("label", TestValue::text("original node")),
                    ),
                )
                .value("peer", TestValue::pointer("owned"))
                .value("self", TestValue::pointer("Owner.One")),
        )
        .build()
}

pub(crate) fn original_nested<'source>(
    forge: &'source DataForge<'source>,
) -> TestResult<RecordTree<'source>> {
    let record = forge.record(0).ok_or("original nested owner")?;
    assert_eq!(
        record.guid().to_string(),
        "00000000-0000-0000-0000-000000005911",
        "actual original owner identity"
    );
    assert_eq!(
        record.stored_file_name()?,
        "owner.xml",
        "the original filename is not reconstructed from its normalized path"
    );
    let structure = record.struct_type()?;
    let declarations: Vec<_> = structure.properties().collect();
    let actual = declarations
        .iter()
        .map(|property| {
            Ok((
                property.name()?,
                property.data_type()?,
                property
                    .target_struct()
                    .ok_or("actual declared target")?
                    .index(),
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        actual,
        [
            ("child", DataType::StrongPointer, 0),
            ("peer", DataType::WeakPointer, 0),
            ("self", DataType::WeakPointer, 1)
        ],
        "the original strong/nested/root pointer declarations are exact"
    );
    let tree = record.tree()?;
    let Some(Value::Struct(child)) = tree.field("child") else {
        return Err("actual original owned Node".into());
    };
    assert_eq!(
        child.type_name(),
        "Node",
        "the original owned value has its own concrete type"
    );
    assert_eq!(
        child.field("label"),
        Some(&Value::String("original node")),
        "the real owned payload was decoded"
    );
    Ok(tree)
}
