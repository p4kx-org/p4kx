// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, RecordTree, Value as Raw, ValueKind};
use p4kx_testkit::DataForgeBuilder;

use crate::c2_sequence_storage_fixture::Case;
use crate::commodity_v3_fixture::TestResult;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;

pub(crate) fn check(database: &DataForgeBuilder, expected: DataType, case: Case) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    declarations(&forge, expected)?;
    let slots = match case {
        Case::TwoOwned | Case::NullChild => 2,
        Case::Empty | Case::NullRoot => 0,
    };
    let pooled = if expected == DataType::StrongPointer {
        slots
    } else {
        0
    };
    assert_eq!(
        forge.value_count(ValueKind::StrongPointer),
        pooled,
        "the actual global pool distinguishes inline and pooled ownership"
    );
    let tree = forge
        .record(0)
        .ok_or("original selected property")?
        .tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "wrong declaration remains a genuinely readable source"
    );
    contents(&tree, case, slots)
}

fn declarations(forge: &DataForge<'_>, expected: DataType) -> TestResult {
    let root = forge
        .structs()
        .find(|item| item.name() == Ok(schema::PROPERTY))
        .ok_or("original property declaration")?;
    let pointer = root.properties().nth(1).ok_or("original display field")?;
    assert_eq!(
        pointer.name()?,
        "displayTransformation",
        "the actual original root field is selected"
    );
    assert_eq!(
        (pointer.data_type()?, pointer.is_array()),
        (DataType::StrongPointer, false),
        "the root declaration is an inline strong pointer"
    );
    assert_eq!(
        pointer
            .target_struct()
            .ok_or("original display target")?
            .name()?,
        schema::BASE,
        "the original declaration targets the Base type"
    );
    let sequence = forge
        .structs()
        .find(|item| item.name() == Ok(schema::SEQUENCE))
        .ok_or("original Sequence declaration")?;
    let field = sequence
        .properties()
        .next()
        .ok_or("original Sequence member")?;
    assert_eq!(
        field.name()?,
        "transformations",
        "the actual Sequence member is selected"
    );
    assert_eq!(
        (field.data_type()?, field.is_array()),
        (expected, true),
        "the original declaration differs before any export"
    );
    assert_eq!(
        field
            .target_struct()
            .ok_or("original array target")?
            .name()?,
        schema::BASE,
        "the original declaration targets the Base type"
    );
    Ok(())
}

fn contents(tree: &RecordTree<'_>, case: Case, slots: usize) -> TestResult {
    if matches!(case, Case::NullRoot) {
        assert_eq!(
            tree.field("displayTransformation"),
            Some(&Raw::Null),
            "the original root pointer is genuinely null"
        );
        return Ok(());
    }
    let display = source::structure(tree.field("displayTransformation"))?;
    assert_eq!(
        display.type_name(),
        schema::SEQUENCE,
        "the readable original root is a Sequence"
    );
    let children = source::array(display.field("transformations"))?;
    assert_eq!(
        children.len(),
        slots,
        "all actual original array slots exist"
    );
    for (index, value) in children.iter().enumerate() {
        if matches!(case, Case::NullChild) && index == 0 {
            assert_eq!(value, &Raw::Null, "the actual first pointer is null");
        } else {
            let owned = source::structure(Some(value))?;
            assert_eq!(
                owned.type_name(),
                schema::BASE,
                "each independently owned child has the original Base type"
            );
            assert!(
                owned.fields().is_empty(),
                "each genuine original Base is present and empty"
            );
        }
    }
    Ok(())
}
