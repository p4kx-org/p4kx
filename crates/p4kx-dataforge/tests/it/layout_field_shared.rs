// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType, RecordTree, Struct, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType};

use crate::layout_field_fixture::TestResult;
use crate::layout_field_raw::{self as raw, Tables};

#[test]
fn shared_property_rows_and_zero_size_classes_have_distinct_logical_slots() -> TestResult {
    let bytes = database()?;
    original_bytes(&bytes)?;
    let forge = DataForge::open(&bytes)?;
    original_schema(&forge)?;
    let record = forge.record(0).ok_or("the original shared-range Record")?;
    assert_eq!(
        record.struct_type()?.index(),
        3,
        "the actual top definition is selected"
    );
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the tree retains its actual original owner"
    );
    fields(&tree)?;
    assert_eq!(
        tree.error_count(),
        0,
        "shared metadata is not itself a source error or a Class cycle"
    );
    Ok(())
}

fn fields(tree: &RecordTree<'_>) -> TestResult {
    assert_eq!(
        tree.fields().len(),
        6,
        "two physical property rows create six inherited occurrences"
    );
    for (index, field) in tree.fields().iter().enumerate() {
        assert_eq!(
            field.name(),
            if index % 2 == 0 { "left" } else { "right" },
            "the same physical declarations retain each separate original occurrence"
        );
        assert_eq!(
            field.storage().data_type(),
            DataType::Class,
            "the actual storage is an inline Class"
        );
        assert_eq!(
            field.storage().target(),
            0,
            "every original zero-size target is Empty"
        );
        assert!(
            !field.storage().is_array(),
            "every original Class occurrence is scalar"
        );
        let Value::Struct(empty) = field.value() else {
            return Err("the original empty Class occurrence must remain a struct".into());
        };
        assert_eq!(
            empty.type_name(),
            "Empty",
            "the actual original target type"
        );
        assert!(
            empty.fields().is_empty(),
            "an actual zero-size Class is not an invented null"
        );
    }
    Ok(())
}

fn database() -> TestResult<Vec<u8>> {
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Empty").declared_size(0))
        .structure(definition("Base", None))
        .structure(definition("Middle", Some("Base")))
        .structure(definition("Top", Some("Middle")))
        .record(TestRecord::new(
            0x8888,
            "Top",
            "Top.Source",
            "source/shared-fields.xml",
        ))
        .build();
    let tables = Tables::read(&bytes)?;
    assert_eq!(
        raw::word(&bytes, 20)?,
        6,
        "the writer supplied the six original rows to compact"
    );
    for index in [2, 3] {
        raw::write_half(&mut bytes, tables.structures + index * 16 + 10, 0)?;
    }
    drop(bytes.drain(tables.properties + 2 * 12..tables.mappings));
    raw::write_word(&mut bytes, 20, 2)?;
    Ok(bytes)
}

fn definition(name: &str, parent: Option<&str>) -> TestStruct {
    let mut structure = TestStruct::new(name)
        .declared_size(0)
        .field("left", TestType::Class("Empty".to_owned()))
        .field("right", TestType::Class("Empty".to_owned()));
    if let Some(parent) = parent {
        structure = structure.parent(parent);
    }
    structure
}

fn original_bytes(bytes: &[u8]) -> TestResult {
    let tables = Tables::read(bytes)?;
    assert_eq!(
        (raw::word(bytes, 16)?, raw::word(bytes, 20)?),
        (4, 2),
        "actual physical table sizes"
    );
    assert_eq!(
        (raw::word(bytes, 28)?, raw::word(bytes, 32)?),
        (4, 1),
        "actual mapping and Record tables"
    );
    assert_eq!(
        bytes
            .get(tables.instances..)
            .ok_or("original instance tail")?
            .len(),
        0,
        "the actual mapped zero-size instance has no payload bytes"
    );
    for index in 1..4 {
        let at = tables.structures + index * 16;
        assert_eq!(
            raw::half(bytes, at + 8)?,
            2,
            "each actual definition declares two properties"
        );
        assert_eq!(
            raw::half(bytes, at + 10)?,
            0,
            "all three actual definitions share the same range"
        );
        assert_eq!(
            raw::word(bytes, at + 12)?,
            0,
            "all actual instance sizes are zero"
        );
    }
    for index in 0..2 {
        let at = tables.properties + index * 12;
        assert_eq!(
            (raw::half(bytes, at + 4)?, raw::half(bytes, at + 6)?),
            (0, 0x0010),
            "both physical properties are real inline Class references to Empty"
        );
    }
    Ok(())
}

fn original_schema(forge: &DataForge<'_>) -> TestResult {
    assert_eq!(
        forge.property_count(),
        2,
        "the reader sees the two physical rows without duplication"
    );
    for (index, structure) in forge.structs().enumerate() {
        original_definition(structure, index)?;
    }
    Ok(())
}

fn original_definition(structure: Struct<'_>, index: usize) -> TestResult {
    let expected = match index {
        0 => "Empty",
        1 => "Base",
        2 => "Middle",
        3 => "Top",
        _ => return Err("unexpected original definition".into()),
    };
    assert_eq!(structure.name()?, expected, "the actual original type");
    assert_eq!(
        structure.size(),
        0,
        "the original type really occupies zero bytes"
    );
    assert_eq!(
        structure.instance_count(),
        u32::from(index == 3),
        "only the actual Top instance is mapped"
    );
    let parent = structure.parent().map(|parent| parent.index());
    let expected_parent = match index {
        0 | 1 => None,
        2 => Some(1),
        _ => Some(2),
    };
    assert_eq!(
        parent, expected_parent,
        "the original parent chain is acyclic"
    );
    original_properties(structure, index != 0)
}

fn original_properties(structure: Struct<'_>, has_fields: bool) -> TestResult {
    let fields: Vec<_> = structure.properties().collect();
    assert_eq!(
        fields.len(),
        if has_fields { 2 } else { 0 },
        "actual own range cardinality"
    );
    for field in fields {
        assert_eq!(
            field.data_type()?,
            DataType::Class,
            "the original declaration's actual type"
        );
        assert!(
            !field.is_array(),
            "the physical declaration is a scalar Class"
        );
        assert_eq!(
            field
                .target_struct()
                .ok_or("original Class target")?
                .index(),
            0,
            "the original target is the separate Empty definition"
        );
    }
    Ok(())
}
