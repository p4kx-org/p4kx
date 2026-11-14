// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::query::QuerySchema;
use p4kx_dataforge::{DataForge, DataType, FieldStatistics, TreeBuilder};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::layout_field_fixture::TestResult;

const OWN: usize = 32_769;

fn database() -> Vec<u8> {
    let mut base = TestStruct::new("WideBase");
    let mut derived = TestStruct::new("WideDerived").parent("WideBase");
    for index in 0..OWN {
        base = base.field(format!("b{index}"), TestType::UInt8);
    }
    for index in 0..OWN - 2 {
        derived = derived.field(format!("d{index}"), TestType::UInt8);
    }
    derived = derived
        .array("targets", TestType::StrongPointer("Target".to_owned()))
        .field("tail", TestType::WeakPointer("Target".to_owned()));
    DataForgeBuilder::new()
        .structure(base)
        .structure(derived)
        .structure(TestStruct::new("Target"))
        .record(
            TestRecord::new(55, "WideDerived", "Wide.Source", "wide/source.xml")
                .value(
                    "targets",
                    TestValue::Array(vec![TestValue::Struct(TestInstance::new("Target"))]),
                )
                .value(
                    "tail",
                    TestValue::Raw([2_u32, 0].into_iter().flat_map(u32::to_le_bytes).collect()),
                ),
        )
        .build()
}

fn original(forge: &DataForge<'_>) -> TestResult {
    assert_eq!(
        (
            forge.struct_count(),
            forge.property_count(),
            forge.record_count()
        ),
        (3, 2 * OWN, 1),
        "the original physical rows are distinct from inherited logical occurrences"
    );
    let record = forge.record(0).ok_or("the actual original Holder")?;
    let child = record.struct_type()?;
    assert_eq!(
        child.properties().count(),
        OWN,
        "actual own count fits its source u16"
    );
    assert_eq!(
        child
            .parent()
            .ok_or("actual original Base")?
            .properties()
            .count(),
        OWN,
        "actual inherited own count also fits u16"
    );
    assert_eq!(
        child.size(),
        u32::try_from(2 * OWN + 14)?,
        "all original byte widths"
    );
    let last = child
        .properties()
        .last()
        .ok_or("the last original declaration")?;
    assert_eq!(
        last.name()?,
        "tail",
        "the high logical ordinal's actual source name"
    );
    assert_eq!(
        last.data_type()?,
        DataType::WeakPointer,
        "the actual tail storage"
    );
    Ok(())
}

#[test]
fn a_pointer_probe_reads_an_array_after_the_inherited_u16_boundary() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    original(&forge)?;
    let probe = forge
        .pointer_probe("targets", "Target")
        .ok_or("the actual target instance")?;
    assert!(
        TreeBuilder::new().holds(&probe, &forge.record(0).ok_or("actual Record")?)?,
        "the original last array is not hidden behind a field quota"
    );
    Ok(())
}

#[test]
fn pointer_offsets_keep_the_actual_last_scalar_slot() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    original(&forge)?;
    let survey = forge.survey()?;
    assert_eq!(
        survey.unreadable_structs, 0,
        "both original instantiated layouts remain readable"
    );
    assert_eq!(
        survey.pointers.narrow, 2,
        "one original pooled strong and one inline weak pointer"
    );
    assert_eq!(survey.pointers.dangling, 0, "both original targets exist");
    Ok(())
}

#[test]
fn statistics_keep_the_last_field_of_the_original_wide_struct() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    original(&forge)?;
    let statistics = FieldStatistics::of_all(&forge)?;
    assert_eq!(
        (
            statistics.records(),
            statistics.unreadable_records(),
            statistics.unreadable_values()
        ),
        (1, 0, 0),
        "the actual wide Record is fully surveyed"
    );
    assert_eq!(
        statistics
            .get("tail")
            .ok_or("the original last path")?
            .occurrences(),
        1,
        "the high original field position reaches the sparse path receiver"
    );
    Ok(())
}

#[test]
fn completion_keeps_the_actual_field_after_the_boundary() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    original(&forge)?;
    let schema = QuerySchema::of(&forge)?;
    let text = "from WideDerived select tai";
    let completion = schema
        .complete(text, text.len())
        .ok_or("the actual last declaration")?;
    assert_eq!(
        completion.candidates().len(),
        1,
        "one matching actual source field"
    );
    assert_eq!(
        completion.candidates()[0].name(),
        "tail",
        "the field keeps its exact source name"
    );
    assert_eq!(
        completion.candidates()[0].detail(),
        Some("WeakPointer Target"),
        "the original target type"
    );
    assert_eq!(completion.more(), 0, "nothing is hidden");
    Ok(())
}
