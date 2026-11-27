// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataType, Value, ValueKind};
use p4kx_testkit::{DataForgeBuilder, TestPool, TestRecord, TestStruct, TestType, TestValue};

use crate::owned_source_controls as controls;
use crate::owned_source_fixture::{self as fixture, TestResult};

#[test]
fn shared_position_prefixes_do_not_make_a_small_valid_owned_tree_unreadable() -> TestResult {
    let bytes = fixture::database(512, 64);
    let forge = DataForge::open(&bytes)?;
    let record = controls::original(&forge, 512, 64)?;
    let tree = record.tree()?;
    controls::complete(&tree, 512, 64)?;
    controls::tail(&tree, &record, 512, 64)?;
    Ok(())
}

#[test]
fn a_valid_scalar_pool_is_materialized_in_full() -> TestResult {
    const ELEMENTS: u32 = 1_048_577;
    let bytes = scalar_database(ELEMENTS);
    let forge = DataForge::open(&bytes)?;
    original_scalar_pool(&forge, ELEMENTS)?;
    let record = forge.record(0).ok_or("original scalar owner")?;
    let tree = record.tree()?;
    let Some(Value::Array(values)) = tree.field("values") else {
        return Err("the valid original array must not be replaced by an error".into());
    };
    assert_eq!(
        values.len(),
        usize::try_from(ELEMENTS)?,
        "every original array slot survives"
    );
    assert!(
        values.iter().all(|value| matches!(value, Value::Int8(0))),
        "all exact original zero bytes are preserved"
    );
    assert_eq!(
        tree.error_count(),
        0,
        "the valid pool creates no error value"
    );
    Ok(())
}

fn scalar_database(elements: u32) -> Vec<u8> {
    let header: Vec<_> = [elements, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Bytes").array("values", TestType::Int8))
        .values(TestPool::Int8, elements)
        .record(
            TestRecord::new(0x7112, "Bytes", "Bytes.Source", "source/values.xml")
                .value("values", TestValue::Raw(header)),
        )
        .build()
}

fn original_scalar_pool(forge: &DataForge<'_>, elements: u32) -> TestResult {
    assert_eq!(
        forge.value_count(ValueKind::Int8),
        usize::try_from(elements)?,
        "the original scalar pool actually contains every authored byte"
    );
    let structure = forge.structs().next().ok_or("original array owner")?;
    assert_eq!(structure.instance_count(), 1, "the original owner exists");
    let property = structure
        .properties()
        .next()
        .ok_or("original array declaration")?;
    assert_eq!(
        property.data_type()?,
        DataType::Int8,
        "original scalar width"
    );
    assert!(property.is_array(), "the source declaration is an array");
    let survey = forge.survey()?;
    assert_eq!(
        survey.unreadable_structs, 0,
        "the original layout is readable"
    );
    assert_eq!(
        (
            survey.pointers.narrow,
            survey.pointers.wide,
            survey.pointers.dangling
        ),
        (0, 0, 0),
        "this source has no pointer or missing-target dependency"
    );
    Ok(())
}
