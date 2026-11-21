// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{SAMPLE, TestResult, node, raw, value};
use p4kx_dataforge::{DataForge, DataType, Field, Value, ValueError, ValueKind};
use p4kx_testkit::{
    DataForgeBuilder, TestInstance, TestPool, TestRecord, TestStruct, TestType, TestValue,
};

#[test]
fn a_long_valid_owned_chain_keeps_every_original_instance() -> TestResult {
    const DEPTH: usize = 69;
    let mut chain = TestValue::Null;
    for _ in 0..DEPTH {
        chain = TestValue::Struct(TestInstance::new("Node").value("next", chain));
    }
    let bytes = DataForgeBuilder::new()
        .structure(node())
        .record(TestRecord::new(SAMPLE, "Node", "Node.One", "node.xml").value("next", chain))
        .build();
    let forge = DataForge::open(&bytes)?;
    let source = forge.structs().next().ok_or("original Node declaration")?;
    assert_eq!(
        source.instance_count(),
        70,
        "all original root and target instances exist"
    );
    let survey = forge.survey()?;
    assert_eq!(
        survey.pointers.narrow, 69,
        "every original nonterminal edge exists"
    );
    assert_eq!(survey.pointers.null, 1, "one genuine terminal null");
    let tree = forge.records().next().ok_or("original Record")?.tree()?;
    let mut depth = 0;
    let mut current = value(&tree, "next")?;
    while let Value::Struct(structure) = current {
        depth += 1;
        current = structure
            .fields()
            .first()
            .map(Field::value)
            .ok_or("actual next field")?;
    }
    assert_eq!(
        depth, DEPTH,
        "all original nested structs below the root survive"
    );
    assert_eq!(current, &Value::Null, "the authored terminal is preserved");
    assert_eq!(tree.error_count(), 0, "valid source nesting is complete");
    Ok(())
}

#[test]
fn a_true_inline_class_cycle_is_an_error_at_each_original_member() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Loop")
                .field("left", TestType::Class("Loop".to_owned()))
                .field("right", TestType::Class("Loop".to_owned()))
                .declared_size(0),
        )
        .record(TestRecord::new(SAMPLE, "Loop", "Loop.One", "loop.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;
    inline_declaration(&forge)?;
    let tree = forge
        .records()
        .next()
        .ok_or("original Loop Record")?
        .tree()?;
    for name in ["left", "right"] {
        assert_eq!(
            value(&tree, name)?,
            &Value::Error(ValueError::InlineClassCycle { struct_index: 0 }),
            "the exact cyclic declaration is retained at {name}"
        );
    }
    assert_eq!(
        tree.error_count(),
        2,
        "both original invalid members are diagnosed"
    );
    let paths: Vec<_> = tree
        .located_errors()
        .map(|(location, _)| location.to_string())
        .collect();
    assert_eq!(
        paths,
        ["left", "right"],
        "each error retains its original location and order"
    );
    Ok(())
}

fn inline_declaration(forge: &DataForge<'_>) -> TestResult {
    let source = forge
        .structs()
        .next()
        .ok_or("the actual inline declaration")?;
    assert_eq!(source.index(), 0, "the actual original struct index");
    assert_eq!(
        source.size(),
        0,
        "zero-width inline declarations are syntactically valid"
    );
    let fields: Vec<_> = source.properties().collect();
    assert_eq!(fields.len(), 2, "both original class declarations exist");
    for (field, expected) in fields.iter().zip(["left", "right"]) {
        assert_eq!(field.name()?, expected, "original declaration order");
        assert_eq!(
            field.data_type()?,
            DataType::Class,
            "an actual inline class"
        );
        assert!(!field.is_array(), "no owned array is involved");
        assert_eq!(
            field.target_struct().ok_or("original target")?.index(),
            0,
            "the actual self edge"
        );
    }
    Ok(())
}

#[test]
fn a_valid_scalar_array_keeps_every_original_element() -> TestResult {
    const COUNT: u32 = 1_048_577;
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Big").array("bytes", TestType::Int8))
        .values(TestPool::Int8, COUNT)
        .record(
            TestRecord::new(SAMPLE, "Big", "Big.One", "big.xml").value("bytes", raw([COUNT, 0])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    assert_eq!(
        forge.value_count(ValueKind::Int8),
        usize::try_from(COUNT)?,
        "the actual complete source pool"
    );
    let tree = forge
        .records()
        .next()
        .ok_or("original Big Record")?
        .tree()?;
    let Value::Array(elements) = value(&tree, "bytes")? else {
        return Err("original complete array".into());
    };
    assert_eq!(
        elements.len(),
        usize::try_from(COUNT)?,
        "every original scalar slot is retained"
    );
    assert!(
        elements.iter().all(|value| matches!(value, Value::Int8(0))),
        "all original pool values survive"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "valid complete source has no fabricated error"
    );
    assert!(
        tree.located_errors().next().is_none(),
        "the valid field has no fabricated error location"
    );
    Ok(())
}
