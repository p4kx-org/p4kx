// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{Query, QueryUnavailable};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;
use super::instance_tests::selector;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn located_pointer_class_array_and_reference_use_the_target_block() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    for condition in [
        "direct.ready = true",
        "weak->ready = true",
        "classes[0].ready = true",
        "linked->ready = true",
    ] {
        let query = Query::parse(&format!("from Owner where {condition}"))?;
        let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
        assert_eq!(
            report.reduction.statistics().records(),
            1,
            "the existing Query reaches a readable target for {condition}"
        );
        assert_target_bound(&report, &forge, 2)?;
        assert!(
            report.unavailable_traversals.is_empty(),
            "the reached target has a real original instance"
        );
        assert!(
            !report.selection_origin.source_set_complete(),
            "the existing proof still leaves arbitrary WHERE coverage incomplete"
        );
    }
    Ok(())
}

#[test]
fn later_unavailable_target_does_not_extend_the_located_block_prefix() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(
        "from Owner where (direct.ready = true and late->ready = true) or fallback = true",
    )?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the later readable condition recovers the unavailable target"
    );
    assert_target_bound(&report, &forge, 2)?;
    assert_failed_target(&report, &forge)
}

fn assert_failed_target(report: &FieldStatisticsReport<'_>, forge: &DataForge<'_>) -> TestResult {
    let late = forge
        .data_mappings()
        .find(|row| row.struct_index == 4)
        .ok_or("the missing instance belongs to a known original block")?;
    assert!(
        selector(
            report,
            "dataforgeDataMapping",
            &[u64::from(late.index)],
            "instanceCount"
        ) && selector(report, "dataforgeStructDefinition", &[4], "size"),
        "the failed target still consulted its own block count and original Struct size"
    );
    assert!(
        !selector(
            report,
            "dataforgeDataMappingRange",
            &[0, u64::from(late.index)],
            "instanceCount"
        ) && !selector(report, "dataforgeStructDefinition", &[3], "size"),
        "an unavailable later instance never uses the intervening block to calculate its offset"
    );
    assert!(
        report.unavailable_traversals.iter().any(|gap| matches!(
            gap, QueryUnavailable::Instance { structure: 4, instance, .. }
                if *instance == u32::from(u16::MAX)
        )),
        "the attempted endpoint retains its exact logical coordinate"
    );
    assert!(
        !report.inventory.iter_uses().any(|site| {
            site.role() == SourceUseRole::Selector && matches!(
                site.location(),
                SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "dataforgeInstance"
                        && indices.as_slice() == [4, u64::from(u16::MAX)] && field == "selection"
            )
        }),
        "the unavailable endpoint has no claimed successful instance use"
    );
    Ok(())
}

#[test]
fn unknown_pointer_struct_does_not_claim_an_original_size_or_mapping() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Owner where unknown.ready = true or fallback = true")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the invalid pointer remains an original attempted operand"
    );
    assert!(
        report.unavailable_traversals.iter().any(|gap| matches!(
            gap,
            QueryUnavailable::Instance {
                structure: 99,
                instance: 0,
                ..
            }
        )),
        "the logical endpoint is identified despite missing source bytes"
    );
    assert!(
        !selector(&report, "dataforgeStructDefinition", &[99], "size")
            && !selector(&report, "dataforgeHeader", &[], "dataMappingCount"),
        "an out-of-range Struct does not consult a nonexistent extent or mapping row"
    );
    Ok(())
}

#[test]
fn typed_pointer_rejection_retains_extent_without_claiming_a_selected_instance() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Owner where pointers[Wanted].ready = true or fallback = true")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the fallback admits the Record after the readable instance is rejected by its type"
    );
    assert_target_bound(&report, &forge, 5)?;
    assert!(
        !selector(&report, "dataforgeInstance", &[5, 0], "selection"),
        "the extent was checked but the type is not a selected predicate instance"
    );
    Ok(())
}

fn assert_target_bound(
    report: &FieldStatisticsReport<'_>,
    forge: &DataForge<'_>,
    structure: u32,
) -> TestResult {
    let target = forge
        .data_mappings()
        .find(|row| row.struct_index == structure)
        .ok_or("the reached target has an original physical mapping")?;
    for field in ["structIndex", "instanceCount"] {
        assert!(
            selector(
                report,
                "dataforgeDataMapping",
                &[u64::from(target.index)],
                field
            ) && selector(
                report,
                "dataforgeDataMappingRange",
                &[0, u64::from(target.index)],
                field
            ),
            "the target row and exact preceding range each supply {field}"
        );
    }
    for type_index in [0, 1, u64::from(structure)] {
        assert!(
            selector(report, "dataforgeStructDefinition", &[type_index], "size"),
            "the checked original Struct widths supply the target address"
        );
    }
    Ok(())
}

fn fixture() -> Vec<u8> {
    let mut unknown = 99_u32.to_le_bytes().to_vec();
    unknown.extend_from_slice(&0_u32.to_le_bytes());
    let owner = TestRecord::new(2, "Owner", "Owner", "libs/owner.xml")
        .value("direct", TestValue::Struct(reached()))
        .value("weak", TestValue::pointer("weakTarget"))
        .value(
            "classes",
            TestValue::Array(vec![TestValue::Struct(reached())]),
        )
        .value(
            "pointers",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new("Other").value("ready", TestValue::Bool(true)),
            )]),
        )
        .value("linked", TestValue::Reference(3))
        .value("late", TestValue::Reference(5))
        .value("unknown", TestValue::Raw(unknown))
        .value("fallback", TestValue::Bool(true));
    DataForgeBuilder::new()
        .structure(TestStruct::new("Before").field("marker", TestType::Double))
        .structure(
            TestStruct::new("Owner")
                .field("direct", TestType::StrongPointer("Reached".into()))
                .field("weak", TestType::WeakPointer("Reached".into()))
                .array("classes", TestType::Class("Reached".into()))
                .array("pointers", TestType::StrongPointer("Reached".into()))
                .field("linked", TestType::ReferenceTo("Reached".into()))
                .field("late", TestType::ReferenceTo("Failed".into()))
                .field("unknown", TestType::StrongPointer("Reached".into()))
                .field("fallback", TestType::Bool),
        )
        .structure(TestStruct::new("Reached").field("ready", TestType::Bool))
        .structure(TestStruct::new("Between").field("marker", TestType::Int32))
        .structure(TestStruct::new("Failed").field("ready", TestType::Bool))
        .structure(TestStruct::new("Other").parent("Reached"))
        .structure(TestStruct::new("Wanted").parent("Reached"))
        .instance("weakTarget", reached())
        .record(TestRecord::new(1, "Before", "Before", "libs/before.xml"))
        .record(owner)
        .record(
            TestRecord::new(3, "Reached", "Reached", "libs/reached.xml")
                .value("ready", TestValue::Bool(true)),
        )
        .record(TestRecord::new(4, "Between", "Between", "libs/between.xml"))
        .record(
            TestRecord::new(5, "Failed", "Failed", "libs/failed.xml").with_instance_index(u16::MAX),
        )
        .build()
}

fn reached() -> TestInstance {
    TestInstance::new("Reached").value("ready", TestValue::Bool(true))
}
