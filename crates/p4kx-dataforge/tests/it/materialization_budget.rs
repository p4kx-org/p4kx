// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, DataForgeError, Step, TreeBuilder, Value, ValueError};

#[path = "materialization_budget_fixture.rs"]
mod fixture;
#[path = "materialization_budget_observer.rs"]
mod observer;
#[path = "layout_field_u64_fixture.rs"]
mod wide_source;

use fixture::TestResult;

#[test]
fn exact_array_allowance_is_complete_and_refusal_does_not_poison_the_builder() -> TestResult {
    let bytes = fixture::array_bytes(3)?;
    fixture::original_array(&bytes, 3)?;
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual original owner")?;
    let mut builder = TreeBuilder::new();
    assert!(
        matches!(
            builder.build_with_budget(&record, 3),
            Err(DataForgeError::MaterializationBudgetExceeded {
                limit: 3,
                required: 4
            })
        ),
        "one Array value plus three actual Struct elements require four slots"
    );
    let exact = builder.build_with_budget(&record, 4)?;
    let ordinary = builder.build(&record)?;
    assert_eq!(
        exact, ordinary,
        "the default builder has no inherited policy allowance"
    );
    assert_eq!(
        exact.error_count(),
        0,
        "all genuine source values are readable"
    );
    let Some(Value::Array(values)) = exact.field("entries") else {
        return Err("the actual complete Array must survive".into());
    };
    assert_eq!(values.len(), 3, "every original element is returned");
    for value in values {
        let Value::Struct(value) = value else {
            return Err("actual Empty Struct element".into());
        };
        assert_eq!(value.type_name(), "Empty", "the genuine Class element type");
        assert!(
            value.fields().is_empty(),
            "each zero-size element still exists as a Struct"
        );
    }
    assert!(
        matches!(
            builder.build_with_budget(&record, 0),
            Err(DataForgeError::MaterializationBudgetExceeded {
                limit: 0,
                required: 1
            })
        ),
        "the original root field needs one output slot before Array reserve"
    );
    assert_eq!(
        builder.build_with_budget(&record, 4)?,
        ordinary,
        "a second refusal does not retain stale ownership, budget or layout state"
    );
    Ok(())
}

#[test]
fn full_u32_zero_class_metadata_is_refused_before_array_capacity_is_reserved() -> TestResult {
    let bytes = crate::layout_field_array_source::database()?;
    let forge = DataForge::open(&bytes)?;
    crate::layout_field_array_source::schema(&forge)?;
    fixture::original_array(&bytes, u32::MAX)?;
    let record = forge.record(0).ok_or("actual huge-count Holder")?;
    assert!(
        matches!(
            TreeBuilder::new().build_with_budget(&record, 1),
            Err(DataForgeError::MaterializationBudgetExceeded {
                limit: 1,
                required: 4_294_967_296
            })
        ),
        "the actual count is checked before any huge Array reserve or traversal"
    );
    Ok(())
}

#[test]
fn an_invalid_source_range_is_one_original_error_instead_of_a_budget_prefix() -> TestResult {
    let mut bytes = fixture::array_bytes(3)?;
    fixture::original_array(&bytes, 3)?;
    let tables = crate::layout_field_raw::Tables::read(&bytes)?;
    crate::layout_field_raw::write_word(&mut bytes, tables.instances, u32::MAX)?;
    assert_eq!(
        crate::layout_field_raw::word(&bytes, tables.mappings)?,
        3,
        "the genuine mapped pool still contains only three instances"
    );
    assert_eq!(
        crate::layout_field_raw::word(&bytes, tables.instances)?,
        u32::MAX,
        "the original selected range is genuinely invalid"
    );
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual invalid-range owner")?;
    let tree = TreeBuilder::new().build_with_budget(&record, 1)?;
    assert_eq!(
        tree.error_count(),
        1,
        "one complete original ValueError is retained"
    );
    assert_eq!(
        tree.field("entries"),
        Some(&Value::Error(ValueError::ArrayOutOfRange {
            first: 0,
            count: u32::MAX,
            len: 3,
            pool: "instance block",
        })),
        "the actual original range facts are preserved before child admission"
    );
    Ok(())
}

#[test]
fn original_bool_error_and_empty_record_remain_distinct_from_resource_refusal() -> TestResult {
    let bytes = fixture::malformed_bool();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual malformed Bool owner")?;
    let original = record.tree()?;
    assert_eq!(
        original.field("flag"),
        Some(&Value::Error(ValueError::InvalidBool { byte: 2 })),
        "the independent original decode observes the exact malformed Bool byte"
    );
    let tree = TreeBuilder::new().build_with_budget(&record, 1)?;
    assert_eq!(
        tree, original,
        "an exact allowance preserves the actual source error tree"
    );
    assert_eq!(
        tree.field("flag"),
        Some(&Value::Error(ValueError::InvalidBool { byte: 2 })),
        "a real malformed value is retained as the same original error"
    );
    assert_eq!(
        tree.error_count(),
        1,
        "source error accounting is unchanged"
    );
    let bytes = fixture::empty();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual Empty owner")?;
    assert_eq!(
        record.struct_type()?.properties().count(),
        0,
        "actual root has no Value slots"
    );
    let tree = TreeBuilder::new().build_with_budget(&record, 0)?;
    assert!(
        tree.root().fields().is_empty(),
        "the implicit Record root consumes no allowance"
    );
    Ok(())
}

#[test]
fn genuine_header_error_precedes_output_admission() -> TestResult {
    const INVALID_OFFSET: u32 = u32::MAX - 1;
    let mut bytes = fixture::array_bytes(3)?;
    fixture::original_array(&bytes, 3)?;
    let tables = crate::layout_field_raw::Tables::read(&bytes)?;
    let blob_size = tables
        .instances
        .checked_sub(tables.blob)
        .ok_or("actual source blob bounds")?;
    assert!(
        u64::from(INVALID_OFFSET) > u64::try_from(blob_size)?,
        "the original name offset is beyond the independently measured actual blob table"
    );
    crate::layout_field_raw::write_word(&mut bytes, tables.records, INVALID_OFFSET)?;
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual header-error owner")?;
    let original = record
        .name()
        .expect_err("the original Record name offset is invalid");
    assert!(
        matches!(
            original,
            DataForgeError::InvalidString {
                offset: INVALID_OFFSET,
                ..
            }
        ),
        "the independent original header control reaches the exact failure"
    );
    assert_eq!(
        TreeBuilder::new()
            .build_with_budget(&record, 0)
            .expect_err("unreadable header"),
        original,
        "a tiny allowance must not replace the actual header failure"
    );
    Ok(())
}

#[test]
fn later_refusal_after_real_owned_children_does_not_reuse_stale_ownership() -> TestResult {
    let bytes = fixture::nested_owned();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual root with an owned child")?;
    let original = record.tree()?;
    assert_eq!(
        original.error_count(),
        0,
        "the complete small original is genuinely readable"
    );
    let Some(Value::Struct(leaf)) = original.field("owned") else {
        return Err("actual owned Leaf".into());
    };
    assert_eq!(
        leaf.field("left"),
        Some(&Value::Bool(true)),
        "actual original child Bool"
    );
    assert_eq!(
        leaf.field("right"),
        Some(&Value::Int32(-7)),
        "actual original child integer"
    );
    assert_eq!(
        original.field("values"),
        Some(&Value::Array(
            vec![Value::UInt8(3), Value::UInt8(0), Value::UInt8(9),].into_boxed_slice()
        )),
        "all three actual later Array elements exist"
    );
    let mut builder = TreeBuilder::new();
    assert!(
        matches!(
            builder.build_with_budget(&record, 5),
            Err(DataForgeError::MaterializationBudgetExceeded {
                limit: 5,
                required: 7
            })
        ),
        "the later Array is refused after the actual owned child was read"
    );
    assert_eq!(
        builder.build_with_budget(&record, 7)?,
        original,
        "all seven original slots survive with an exact allowance after refusal"
    );
    assert_eq!(
        builder.build(&record)?,
        original,
        "the next unbudgeted read has no stale allowance or ownership"
    );
    Ok(())
}

#[test]
fn all_wide_fields_are_complete_with_an_exact_allowance_and_without_a_default_quota() -> TestResult
{
    const OWN_FIELDS: usize = 35_000;
    let bytes = crate::layout_field_fixture::database(OWN_FIELDS, 0)?;
    crate::layout_field_raw::original(&bytes, OWN_FIELDS, 0)?;
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual inherited wide owner")?;
    assert_eq!(
        record.type_name()?,
        crate::layout_field_fixture::DERIVED,
        "the original Record selects the real inherited definition"
    );
    let required_values = u64::try_from(2 * OWN_FIELDS)?;
    let mut builder = TreeBuilder::new();
    assert!(
        matches!(builder.build_with_budget(&record, required_values - 1),
        Err(DataForgeError::MaterializationBudgetExceeded { limit, required })
            if limit == required_values - 1 && required == required_values),
        "all actual root fields are admitted before their output reserve"
    );
    let exact = builder.build_with_budget(&record, required_values)?;
    assert_eq!(
        exact.root().fields().len(),
        2 * OWN_FIELDS,
        "no inherited field is truncated"
    );
    assert_eq!(
        exact.error_count(),
        0,
        "the complete actual source remains readable"
    );
    for (index, field) in exact.root().fields().iter().enumerate() {
        assert_eq!(
            field.name(),
            crate::layout_field_fixture::field_name(index, OWN_FIELDS),
            "every actual original field keeps its order and name"
        );
        assert_eq!(
            field.value(),
            &Value::UInt8(crate::layout_field_fixture::value(index, OWN_FIELDS)),
            "every original source byte survives exactly once"
        );
    }
    assert_eq!(
        builder.build(&record)?,
        exact,
        "ordinary materialization has no fixed field quota"
    );
    Ok(())
}

#[test]
fn compact_valid_original_field_count_above_u32_is_refused_only_by_the_caller_allowance()
-> TestResult {
    let bytes = wide_source::database(p4kx_dataforge::SNIFF_LEN)?;
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual compact inherited source")?;
    assert_eq!(
        record.struct_type()?.index(),
        wide_source::SEGMENTS,
        "the actual Record selects the final original parent segment"
    );
    assert_eq!(
        record.struct_type()?.size(),
        0,
        "all actual Class fields have zero stored width"
    );
    let required_values = u64::from(wide_source::SEGMENTS) * u64::from(wide_source::OWN);
    assert!(
        required_values > u64::from(u32::MAX),
        "the genuine logical count exceeds the retired position width"
    );
    assert!(
        matches!(TreeBuilder::new().build_with_budget(&record, 1),
        Err(DataForgeError::MaterializationBudgetExceeded { limit: 1, required })
            if required == required_values),
        "the caller allowance refuses before field output reserve without a hidden u32 quota"
    );
    Ok(())
}

#[test]
fn actual_original_weak_instance_receipts_keep_their_full_tuple_after_budgeted_decode() -> TestResult
{
    let bytes = crate::owned_source_fixture::database(2, 2);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual weak-instance owner")?;
    let original = record.tree()?;
    let tree = TreeBuilder::new().build_with_budget(&record, 12)?;
    assert_eq!(
        tree, original,
        "all twelve actual original Value slots survive unchanged"
    );
    let Some(Value::Reference(reference)) = tree.field("tail") else {
        return Err("actual decoded weak instance Reference".into());
    };
    let receipt = reference.original_target();
    assert!(
        receipt.shares_source_with(&record),
        "the genuine receipt retains the actual original owner"
    );
    let expected: &[u64] = &[0, 1, 1];
    assert_eq!(
        receipt.instance_positions(),
        Some(expected),
        "the actual original field/array/field tuple uses the complete position representation"
    );
    let location = receipt
        .instance_location()
        .ok_or("actual original instance Location")?;
    assert_eq!(
        location.as_str(),
        "roots[1].next",
        "the original logical location is unchanged"
    );
    assert_eq!(
        location.steps(),
        &[Step::Field("roots"), Step::Element(1), Step::Field("next")],
        "array steps retain their actual format-bounded element type"
    );
    Ok(())
}
