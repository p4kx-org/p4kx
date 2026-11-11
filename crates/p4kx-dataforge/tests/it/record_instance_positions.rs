// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use p4kx_dataforge::{DataForge, Guid, Location, Record, RecordTree, Reference, Value, ValueError};
use serde_json::{Value as JsonValue, json};

use crate::record_field_storage_source::{self as storage, TestResult};
use crate::record_instance_position_source as fixture;
use crate::record_reference_origin_source as original;

#[test]
fn weak_instance_receipts_distinguish_actual_duplicate_field_positions() -> TestResult {
    let bytes = fixture::duplicates()?;
    let forge = DataForge::open(&bytes)?;
    let tree = fixture::original_duplicates(&forge)?;
    let owner = forge.record(0).ok_or("actual original owner")?;
    let first = original::reference(tree.field("first").ok_or("first original pointer")?)?;
    let second = original::reference(tree.field("second").ok_or("second original pointer")?)?;
    assert_position(first, &owner, &[1], "slotA")?;
    assert_position(second, &owner, &[2], "slotA")?;
    assert_eq!(
        first, second,
        "physical receipts do not alter legacy five-field Reference equality"
    );
    assert_eq!(
        first.location(),
        second.location(),
        "logical Location equality excludes physical receipts"
    );
    canonical_wire(&tree)
}

fn assert_position(
    reference: &Reference<'_>,
    owner: &Record<'_>,
    expected: &[u64],
    logical: &str,
) -> TestResult {
    let receipt = reference.original_target();
    assert!(
        receipt.shares_source_with(owner),
        "the actual original admission belongs to this Forge"
    );
    assert_eq!(
        receipt.record_index(),
        owner.index(),
        "the original instance retains its owning Record"
    );
    assert_eq!(
        receipt.record_guid(),
        owner.guid(),
        "the instance GUID comes from its actual owner"
    );
    assert_eq!(
        receipt.record_type(),
        None,
        "an instance is not an owning Record type header"
    );
    assert_eq!(
        receipt.instance_positions(),
        Some(expected),
        "exact original field/array tuple"
    );
    let location = receipt
        .instance_location()
        .ok_or("original admitted instance location")?;
    assert_eq!(
        location.as_str(),
        logical,
        "logical Location remains unchanged"
    );
    assert_eq!(
        reference.type_name(),
        "Node",
        "the actual instance type is retained"
    );
    Ok(())
}

fn canonical_wire(tree: &RecordTree<'_>) -> TestResult {
    let (bytes, _) = storage::rendering(tree)?;
    let value: JsonValue = serde_json::from_slice(&bytes)?;
    let expected = json!({
        "guid": tree.guid().to_string(),
        "path": tree.record_path(),
        "type": "Node",
        "at": "slotA",
    });
    for field in ["first", "second"] {
        assert_eq!(
            value.get(field),
            Some(&expected),
            "canonical reference {field} keeps exactly its existing wire fields"
        );
    }
    Ok(())
}

#[test]
fn weak_instance_receipts_keep_inherited_fields_and_original_array_edges() -> TestResult {
    let bytes = fixture::nested();
    let forge = DataForge::open(&bytes)?;
    let tree = fixture::original_nested(&forge)?;
    let owner = forge.record(0).ok_or("actual nested owner")?;
    let Some(Value::Array(links)) = tree.field("links") else {
        return Err("actual original weak array".into());
    };
    assert_eq!(links.len(), 4, "all original repeated/null slots remain");
    let first = original::reference(links.first().ok_or("actual first weak slot")?)?;
    let nested = original::reference(links.get(1).ok_or("actual nested weak slot")?)?;
    let repeated = original::reference(links.get(2).ok_or("actual repeated weak slot")?)?;
    assert_position(first, &owner, &[1, 1], "nodes[1]")?;
    assert_position(nested, &owner, &[2, 0], "wrapper.inner")?;
    assert_position(repeated, &owner, &[1, 1], "nodes[1]")?;
    assert!(
        std::ptr::eq(
            first.location().ok_or("first actual Location")?,
            repeated.location().ok_or("repeated actual Location")?
        ),
        "the existing cache shares one original Location across repeated actual target admissions"
    );
    assert_eq!(
        links.get(3),
        Some(&Value::Null),
        "the original null array slot does not gain an admission"
    );
    Ok(())
}

#[test]
fn instance_receipts_preserve_clone_semantics_without_granting_foreign_authority() -> TestResult {
    let bytes = fixture::duplicates()?;
    let first_forge = DataForge::open(&bytes)?;
    let second_forge = DataForge::open(&bytes)?;
    let first_tree = fixture::original_duplicates(&first_forge)?;
    let second_tree = fixture::original_duplicates(&second_forge)?;
    let first_owner = first_forge.record(0).ok_or("first actual owner")?;
    let second_owner = second_forge.record(0).ok_or("second actual owner")?;
    let first = original::reference(first_tree.field("first").ok_or("first original link")?)?;
    let second = original::reference(second_tree.field("first").ok_or("second original link")?)?;
    foreign_receipts(first, second, &first_owner, &second_owner);
    cloned_receipt(first, &first_owner)?;
    semantic_location(first)?;
    assert_eq!(
        storage::rendering(&first_tree)?,
        storage::rendering(&second_tree)?,
        "original receipts do not change canonical JSON/XML"
    );
    Ok(())
}

fn foreign_receipts(
    first: &Reference<'_>,
    second: &Reference<'_>,
    owner: &Record<'_>,
    foreign: &Record<'_>,
) {
    assert_eq!(first, second, "all legacy target descriptors can coincide");
    assert_eq!(
        first.original_target().instance_positions(),
        second.original_target().instance_positions(),
        "equal physical tuples are still only descriptions"
    );
    assert!(
        first.original_target().shares_source_with(owner),
        "the original admission keeps its real owner"
    );
    assert!(
        !first.original_target().shares_source_with(foreign),
        "matching descriptors and tuples do not authorize another Forge"
    );
    assert!(
        !second.original_target().shares_source_with(owner),
        "the other genuine admission stays foreign"
    );
}

fn cloned_receipt(reference: &Reference<'_>, owner: &Record<'_>) -> TestResult {
    let cloned = (*reference).clone();
    let original = reference.original_target();
    let receipt = cloned.original_target();
    assert_eq!(
        &cloned, reference,
        "whole original clone keeps legacy value equality"
    );
    assert!(
        receipt.shares_source_with(owner),
        "whole original clone preserves the original owner"
    );
    assert!(
        std::ptr::eq(
            original.instance_positions().ok_or("original positions")?,
            receipt.instance_positions().ok_or("cloned positions")?
        ),
        "Reference clone shares the original existing Location allocation and positions"
    );
    Ok(())
}

fn semantic_location(reference: &Reference<'_>) -> TestResult {
    let location = reference
        .location()
        .ok_or("genuine original instance Location")?;
    let logical = Location::new(location.steps());
    assert_eq!(
        location, &logical,
        "public logical Location equality ignores private positions"
    );
    assert_eq!(
        hash(location),
        hash(&logical),
        "Location Hash remains consistent with its existing value equality"
    );
    assert_eq!(
        format!("{location:?}"),
        format!("{logical:?}"),
        "original positions do not enter Debug"
    );
    assert_eq!(
        format!("{location:?}"),
        r#"Location { steps: [Field("slotA")], text: "slotA" }"#,
        "Debug keeps exactly the original field names and semantic values"
    );
    assert_eq!(
        location.to_string(),
        logical.to_string(),
        "Display retains the existing logical path"
    );
    Ok(())
}

fn hash(location: &Location<'_>) -> u64 {
    let mut state = DefaultHasher::new();
    location.hash(&mut state);
    state.finish()
}

#[test]
fn record_and_failed_pointer_slots_do_not_fabricate_instance_positions() -> TestResult {
    let bytes = fixture::nested();
    let forge = DataForge::open(&bytes)?;
    let tree = fixture::original_nested(&forge)?;
    for field in ["self", "record"] {
        let reference = original::reference(tree.field(field).ok_or("actual Record target slot")?)?;
        let receipt = reference.original_target();
        assert_eq!(
            receipt.instance_location(),
            None,
            "{field} admits the Record itself"
        );
        assert_eq!(
            receipt.instance_positions(),
            None,
            "{field} has no owned-instance tuple"
        );
        assert_eq!(
            receipt.record_type(),
            Some("Owner"),
            "{field} retains original Record applicability"
        );
    }
    unresolved_slots(&tree)
}

fn unresolved_slots(tree: &RecordTree<'_>) -> TestResult {
    assert_eq!(
        tree.field("empty"),
        Some(&Value::Null),
        "the original null pointer remains null"
    );
    assert_eq!(
        tree.field("invalid"),
        Some(&Value::Error(ValueError::InvalidPointer {
            struct_index: fixture::INVALID_STRUCT,
            instance_index: fixture::INVALID_INSTANCE,
        })),
        "the original invalid words remain a typed pointer failure"
    );
    let missing: Guid = "00000000-0000-0000-0000-000000005a12".parse()?;
    assert_eq!(
        tree.field("missing"),
        Some(&Value::Error(ValueError::DanglingReference {
            guid: missing
        })),
        "an original dangling GUID never becomes an instance admission"
    );
    Ok(())
}
