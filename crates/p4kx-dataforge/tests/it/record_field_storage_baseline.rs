// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::DataForge;

use crate::record_field_storage_source::{self as source, Case, TestResult};

#[test]
fn original_reference_and_weak_scalar_links_have_equal_values_and_wire() -> TestResult {
    same_values_and_wire(false)
}

#[test]
fn original_reference_and_weak_arrays_preserve_equal_null_and_repeated_slots() -> TestResult {
    same_values_and_wire(true)
}

fn same_values_and_wire(array: bool) -> TestResult {
    let authored = source::database(Case::Reference, array);
    let pointer = source::database(Case::Weak, array);
    let authored = DataForge::open(&authored)?;
    let pointer = DataForge::open(&pointer)?;
    source::original(&authored, Case::Reference, array)?;
    source::original(&pointer, Case::Weak, array)?;
    let first = authored
        .record(0)
        .ok_or("original Reference owner")?
        .tree()?;
    let second = pointer
        .record(0)
        .ok_or("original WeakPointer owner")?
        .tree()?;
    source::equal_links(&first, &second, array)?;
    assert_eq!(
        first, second,
        "tree value equality does not distinguish original storage"
    );
    assert_eq!(
        source::rendering(&first)?,
        source::rendering(&second)?,
        "Record JSON/XML represent identical values independently of physical link storage"
    );
    Ok(())
}
