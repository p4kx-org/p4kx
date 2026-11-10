// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::alloc::Layout;
use std::io;

use p4kx_dataforge::{
    DataForge, Location, Record, RecordTree, Reference, ReferenceOrigin, Value, ValueError,
};
use p4kx_testkit::TestValue;

use crate::record_field_storage_source::{self as fixture, Case, TestResult};
use crate::record_reference_origin_source as original;

#[test]
fn original_target_receipts_keep_actual_lookup_owners_for_scalar_and_array_links() -> TestResult {
    report_layout()?;
    for array in [false, true] {
        for case in [Case::Reference, Case::Weak] {
            let bytes = fixture::database(case, array);
            let forge = DataForge::open(&bytes)?;
            fixture::original(&forge, case, array)?;
            let owner = forge.record(0).ok_or("actual original link owner")?;
            let target = forge.record(1).ok_or("actual original target")?;
            let tree = owner.tree()?;
            assert!(
                tree.is_from(&owner),
                "actual whole-tree pairing precedes target capture"
            );
            admitted_records(&tree, &owner, &target, array)?;
        }
    }
    Ok(())
}

fn record_target(reference: &Reference<'_>, owner: &Record<'_>, target: &Record<'_>) -> TestResult {
    let index = target.index();
    let guid = target.guid();
    let path = target.record_path()?;
    let source_type = target.type_name()?;
    let admitted = reference.original_target();
    assert!(
        admitted.shares_source_with(owner),
        "the demanded target belongs to the actual caller Forge"
    );
    assert!(
        admitted.shares_source_with(target),
        "the actual original target shares the admitted Forge"
    );
    assert_eq!(
        admitted.record_index(),
        index,
        "the target's physical source position is exact"
    );
    assert_eq!(
        admitted.record_guid(),
        guid,
        "the actual original target header supplies its identity"
    );
    assert_eq!(
        admitted.record_path(),
        path,
        "the actual admitted target supplied this normalized path"
    );
    assert_eq!(
        admitted.record_type(),
        Some(source_type),
        "the actual admitted Record has this source type"
    );
    assert_eq!(
        admitted.instance_location(),
        None,
        "both resolved Record link storage kinds admit the Record itself"
    );
    Ok(())
}

#[test]
fn equal_target_descriptors_preserve_foreign_owner_rejection_clone_eq_and_debug() -> TestResult {
    for case in [Case::Reference, Case::Weak] {
        let bytes = fixture::database(case, false);
        let first = DataForge::open(&bytes)?;
        let second = DataForge::open(&bytes)?;
        fixture::original(&first, case, false)?;
        fixture::original(&second, case, false)?;
        let owner = first.record(0).ok_or("first original owner")?;
        let foreign = second.record(0).ok_or("second original owner")?;
        let left = owner.tree()?;
        let right = foreign.tree()?;
        let first = original::reference(left.field("Link").ok_or("first original link")?)?;
        let second = original::reference(right.field("Link").ok_or("second original link")?)?;
        separate_owners(first, second, &owner, &foreign);
        assert_eq!(
            fixture::rendering(&left)?,
            fixture::rendering(&right)?,
            "original target binding never enters canonical JSON/XML"
        );
    }
    Ok(())
}

fn separate_owners(
    first: &Reference<'_>,
    second: &Reference<'_>,
    owner: &Record<'_>,
    foreign: &Record<'_>,
) {
    assert_eq!(
        first, second,
        "visible Reference equality still ignores original authority"
    );
    assert!(
        first.original_target().shares_source_with(owner),
        "the genuine first admission is bound"
    );
    assert!(
        !first.original_target().shares_source_with(foreign),
        "equal descriptors do not retarget the actual original admission"
    );
    assert!(
        !second.original_target().shares_source_with(owner),
        "a foreign actual admission cannot authorize the first input"
    );
    let cloned = (*first).clone();
    assert_eq!(
        &cloned, first,
        "whole genuine clone retains the old value semantics"
    );
    assert!(
        cloned.original_target().shares_source_with(owner),
        "whole clone preserves its original paired owner"
    );
    assert!(
        !cloned.original_target().shares_source_with(foreign),
        "cloning never grants foreign authority"
    );
    old_debug(first);
    old_debug(second);
}

fn old_debug(reference: &Reference<'_>) {
    assert_eq!(
        format!("{reference:?}"),
        r#"Reference { record_index: 1, guid: Guid(00000000-0000-0000-0000-000000005812), record_path: "target", type_name: "Target", location: None }"#,
        "Reference Debug retains exactly its original five descriptive fields"
    );
}

#[test]
fn weak_root_and_owned_instance_admissions_keep_header_applicability_distinct() -> TestResult {
    let bytes = original::nested();
    let forge = DataForge::open(&bytes)?;
    let tree = original::original_nested(&forge)?;
    let owner = forge.record(0).ok_or("actual original nested owner")?;
    assert!(
        tree.is_from(&owner),
        "the original nested tree belongs to the actual owner"
    );
    let root = original::reference(tree.field("self").ok_or("actual weak root link")?)?;
    record_target(root, &owner, &owner)?;
    let instance = original::reference(tree.field("peer").ok_or("actual weak instance link")?)?;
    assert_eq!(
        instance.type_name(),
        "Node",
        "the retained value is the actual concrete instance type"
    );
    let admitted = instance.original_target();
    assert!(
        admitted.shares_source_with(&owner),
        "the original instance admission retains its real owner"
    );
    assert_eq!(
        admitted.record_index(),
        owner.index(),
        "the nested instance retains its owning Record position"
    );
    assert_eq!(
        admitted.record_guid(),
        owner.guid(),
        "the nested instance GUID is the actual owner's header"
    );
    assert_eq!(
        admitted.record_path(),
        owner.record_path()?,
        "the instance path belongs to its original owner"
    );
    assert_eq!(
        admitted.record_type(),
        None,
        "an actual instance type cannot impersonate a Record header type"
    );
    assert_eq!(
        admitted
            .instance_location()
            .map(Location::as_str),
        Some("child"),
        "the actual same-parse owned-instance location survives"
    );
    Ok(())
}

#[test]
fn original_null_and_dangling_slots_do_not_materialize_a_target_admission() -> TestResult {
    let missing = Value::Error(ValueError::DanglingReference {
        guid: "00000000-0000-0000-0000-000000005813".parse()?,
    });
    for (value, expected) in [
        (TestValue::Null, Value::Null),
        (TestValue::Reference(fixture::MISSING), missing),
    ] {
        let bytes = fixture::builder(Case::Reference, false, value).build();
        let forge = DataForge::open(&bytes)?;
        fixture::original(&forge, Case::Reference, false)?;
        let owner = forge.record(0).ok_or("actual original unresolved owner")?;
        let tree = owner.tree()?;
        assert!(
            tree.is_from(&owner),
            "the unresolved source still has its actual owner"
        );
        let value = tree
            .field("Link")
            .ok_or("actual original unresolved slot")?;
        assert_eq!(
            value, &expected,
            "the exact original null or dangling token never becomes a synthesized Reference object"
        );
    }
    Ok(())
}

fn report_layout() -> io::Result<()> {
    use std::io::Write as _;
    writeln!(
        io::stderr().lock(),
        "REFERENCE_ORIGIN_LAYOUT arch={} os={} reference={}/{} view={}/{} value={}/{}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        Layout::new::<Reference<'_>>().size(),
        Layout::new::<Reference<'_>>().align(),
        Layout::new::<ReferenceOrigin<'_, '_>>().size(),
        Layout::new::<ReferenceOrigin<'_, '_>>().align(),
        Layout::new::<Value<'_>>().size(),
        Layout::new::<Value<'_>>().align(),
    )?;
    writeln!(
        io::stderr().lock(),
        "LOCATION_ORIGIN_LAYOUT arch={} os={} location={}/{}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        Layout::new::<Location<'_>>().size(),
        Layout::new::<Location<'_>>().align(),
    )
}

fn admitted_records(
    tree: &RecordTree<'_>,
    owner: &Record<'_>,
    target: &Record<'_>,
    array: bool,
) -> TestResult {
    let values = original::values(tree, array)?;
    assert_eq!(
        values.len(),
        if array { 3 } else { 1 },
        "the actual source slots cannot disappear before target admission"
    );
    assert_eq!(
        values
            .iter()
            .filter(|value| matches!(value, Value::Reference(_)))
            .count(),
        if array { 2 } else { 1 },
        "the actual repeated target admissions are nonvacuous"
    );
    if array {
        assert_eq!(
            values.get(1),
            Some(&Value::Null),
            "the original null slot remains separate from target admissions"
        );
    }
    for value in values {
        if matches!(value, Value::Null) {
            continue;
        }
        record_target(original::reference(value)?, owner, target)?;
    }
    Ok(())
}
