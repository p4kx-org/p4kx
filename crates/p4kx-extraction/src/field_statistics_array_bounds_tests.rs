// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::Query;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn value_pool_check_retains_the_constant_original_word_on_success_and_failure() -> TestResult {
    let bytes = fixture().omit_empty_mappings().build();
    let forge = DataForge::open(&bytes)?;
    for (text, attempted) in [
        (r#"from Owner at "libs/good" where numbers[0] > 1"#, false),
        (
            r#"from Owner at "libs/bad" where numbers[0] > 1 or fallback = true"#,
            true,
        ),
    ] {
        let query = Query::parse(text)?;
        let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
        assert_eq!(
            report.reduction.statistics().records(),
            1,
            "the ordinary predicate and recovery admit the expected original Record"
        );
        assert_eq!(
            pool_word(&bytes, &report)?,
            3,
            "the actual Int32 pool count word supplies the range check"
        );
        assert_eq!(
            report.inventory.iter_uses().any(|site| {
                report.selection_attempts.contains(site.id())
                    && site.role() == SourceUseRole::Attempted
                    && matches!(site.location(), SourceLocation::Subresource { field, .. } if field == "arrayHeader")
            }),
            attempted,
            "only the invalid authored array header produces an attempted extent"
        );
        assert!(
            !report.selection_origin.source_set_complete(),
            "other WHERE proof obligations remain separate"
        );
    }
    Ok(())
}

#[test]
fn class_range_uses_the_exact_mapping_row_or_the_proved_absence() -> TestResult {
    let bytes = fixture().omit_empty_mappings().build();
    let forge = DataForge::open(&bytes)?;
    let source = forge
        .data_mappings()
        .find(|row| row.struct_index == 0)
        .ok_or("the present class has a real block")?;
    assert_eq!(
        source.instance_count, 1,
        "the fixture contains one real class instance"
    );
    assert!(
        forge.data_mappings().all(|row| row.struct_index != 1),
        "the unused class has no original mapping row"
    );
    let query = Query::parse("from Owner where occupied[0].ready = true")?;
    let present = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        present.reduction.statistics().records(),
        1,
        "the actual class element satisfies WHERE"
    );
    for name in ["structIndex", "instanceCount"] {
        assert!(
            selector(
                &present,
                "dataforgeDataMapping",
                &[u64::from(source.index)],
                name
            ),
            "the original mapping row supplies {name}"
        );
    }
    assert_missing_mapping(&forge)
}

fn assert_missing_mapping(forge: &DataForge<'_>) -> TestResult {
    let query = Query::parse("from Owner where unmapped[0].ready = true or fallback = true")?;
    let absent = FieldStatisticsReport::query(forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        absent.reduction.statistics().records(),
        2,
        "the missing mapping represents an empty class block"
    );
    assert!(
        selector(&absent, "dataforgeHeader", &[], "dataMappingCount"),
        "the original mapping census declares its bound"
    );
    assert!(
        selector(
            &absent,
            "dataforgeDataMappingRange",
            &[0, forge.data_mapping_count() as u64],
            "structIndex"
        ),
        "the full checked type column proves that no physical row exists"
    );
    let root = forge
        .data_mappings()
        .find(|row| row.struct_index == 2)
        .ok_or("the actual predicate root has a block")?;
    assert!(
        absent.inventory.iter_uses().all(|site| {
            !matches!(site.location(), SourceLocation::Subresource { namespace, indices, field }
                if namespace == "dataforgeDataMapping" && field == "instanceCount"
                    && indices.as_slice() != [u64::from(root.index)])
        }),
        "the absent class adds no count row beyond the real predicate-root mapping"
    );
    Ok(())
}

#[test]
fn explicit_zero_mapping_is_a_physical_bound() -> TestResult {
    let bytes = fixture().build();
    let forge = DataForge::open(&bytes)?;
    let zero = forge
        .data_mappings()
        .find(|row| row.struct_index == 1)
        .ok_or("the fixture deliberately retains the authored zero mapping")?;
    assert_eq!(
        zero.instance_count, 0,
        "no instances in this explicitly mapped block"
    );
    let query = Query::parse("from Owner where unmapped[0].ready = true or fallback = true")?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert!(
        selector(
            &report,
            "dataforgeDataMapping",
            &[u64::from(zero.index)],
            "instanceCount"
        ),
        "authored zero uses its actual mapping row"
    );
    assert!(
        !selector(&report, "dataforgeHeader", &[], "dataMappingCount"),
        "a present mapping does not need the full negative census"
    );
    Ok(())
}

fn pool_word(bytes: &[u8], report: &FieldStatisticsReport<'_>) -> Result<u32, Box<dyn Error>> {
    let address = report
        .inventory
        .iter_uses()
        .find_map(|site| match site.location() {
            SourceLocation::Subresource {
                namespace,
                indices,
                field,
            } if namespace == "dataforgeDecodedBytes"
                && field == "poolCount"
                && report
                    .selection_origin
                    .source_uses_refs()
                    .contains(site.id()) =>
            {
                Some(indices.as_slice())
            }
            _ => None,
        })
        .ok_or("the checked pool count has original byte coordinates")?;
    let [start, end] = address else {
        return Err("the original pool count address is one span".into());
    };
    let span = usize::try_from(*start)?..usize::try_from(*end)?;
    Ok(u32::from_le_bytes(
        bytes
            .get(span)
            .ok_or("the original word exists")?
            .try_into()?,
    ))
}

fn selector(
    report: &FieldStatisticsReport<'_>,
    want_ns: &str,
    want_indices: &[u64],
    want_field: &str,
) -> bool {
    report.inventory.iter_uses().any(|site| {
        site.role() == SourceUseRole::Selector
            && report.selection_origin.source_uses_refs().contains(site.id())
            && matches!(site.location(), SourceLocation::Subresource { namespace, indices, field }
                if namespace == want_ns && indices.as_slice() == want_indices && field == want_field)
    })
}

fn fixture() -> DataForgeBuilder {
    let mut invalid_range = 1_u32.to_le_bytes().to_vec();
    invalid_range.extend_from_slice(&64_u32.to_le_bytes());
    DataForgeBuilder::new()
        .structure(TestStruct::new("Present").field("ready", TestType::Bool))
        .structure(TestStruct::new("Missing").field("ready", TestType::Bool))
        .structure(
            TestStruct::new("Owner")
                .array("numbers", TestType::Int32)
                .array("occupied", TestType::Class("Present".into()))
                .array("unmapped", TestType::Class("Missing".into()))
                .field("fallback", TestType::Bool),
        )
        .record(
            TestRecord::new(1, "Owner", "Good", "libs/good.xml")
                .value(
                    "numbers",
                    TestValue::Array(vec![
                        TestValue::Int32(2),
                        TestValue::Int32(3),
                        TestValue::Int32(4),
                    ]),
                )
                .value(
                    "occupied",
                    TestValue::Array(vec![TestValue::Struct(
                        TestInstance::new("Present").value("ready", TestValue::Bool(true)),
                    )]),
                )
                .value("fallback", TestValue::Bool(true)),
        )
        .record(
            TestRecord::new(2, "Owner", "Bad", "libs/bad.xml")
                .value("numbers", TestValue::Raw(invalid_range))
                .value("fallback", TestValue::Bool(true)),
        )
}
