// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataForgeError, DataType, RecordTree, Value};

use crate::layout_field_fixture as fixture;
use crate::layout_field_original as original;
use crate::layout_field_raw as raw;
use fixture::TestResult;

#[test]
fn two_valid_inheritance_definitions_retain_sixty_five_thousand_five_hundred_thirty_eight_fields()
-> TestResult {
    complete(32_769)
}

#[test]
fn two_valid_inheritance_definitions_retain_the_existing_count_boundary() -> TestResult {
    complete(32_768)
}

fn complete(own: usize) -> TestResult {
    let bytes = fixture::database(own, 0)?;
    raw::original(&bytes, own, 0)?;
    let forge = DataForge::open(&bytes)?;
    let record = original::record(&forge, own, 0)?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the complete tree retains the actual original owner"
    );
    fields(&tree, own);
    assert_eq!(tree.error_count(), 0, "all original UInt8 fields are valid");
    Ok(())
}

fn fields(tree: &RecordTree<'_>, own: usize) {
    assert_eq!(
        tree.fields().len(),
        2 * own,
        "every original inherited and own slot remains"
    );
    for (index, field) in tree.fields().iter().enumerate() {
        assert_eq!(
            field.name(),
            fixture::field_name(index, own),
            "the original total declaration order"
        );
        assert_eq!(
            field.value(),
            &Value::UInt8(fixture::value(index, own)),
            "the exact original instance byte at every slot"
        );
        assert_eq!(
            field.storage().data_type(),
            DataType::UInt8,
            "original scalar storage"
        );
        assert!(
            !field.storage().is_array(),
            "the actual declaration is not an array"
        );
    }
}

#[test]
fn validation_first_reports_the_actual_wide_layout_mismatch() -> TestResult {
    mismatch(32_769)
}

#[test]
fn an_actual_small_layout_mismatch_has_the_same_specific_format_error() -> TestResult {
    mismatch(2)
}

fn mismatch(own: usize) -> TestResult {
    let bytes = fixture::database(own, 1)?;
    raw::original(&bytes, own, 1)?;
    let forge = DataForge::open(&bytes)?;
    let record = original::record(&forge, own, 1)?;
    let error = record
        .tree()
        .err()
        .ok_or("the actual padded layout must remain invalid")?;
    assert_eq!(
        error,
        DataForgeError::StructLayout {
            struct_index: 1,
            declared: u32::try_from(2 * own + 1)?,
            fields: u64::try_from(2 * own)?,
        },
        "the proposed validation-first contract reports the actual original size mismatch"
    );
    Ok(())
}
