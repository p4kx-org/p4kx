// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::query::{Query, ReferenceCoverage};
use p4kx_dataforge::{DataForge, FieldStatistics, Number};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseDescription, SourceUseRef, SourceUseRole};

use super::{FieldStatisticsReport, FieldStatisticsRowProof, FieldStatisticsSelectionProof};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn ties_in_display_paths_keep_original_comparison_operands() -> TestResult {
    let bytes = fixture(false);
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    assert_eq!(
        report.reduction.statistics(),
        &FieldStatistics::of_all(&forge)?,
        "observing actual operands leaves the ordinary statistics unchanged"
    );
    assert_eq!(
        report.selection_proof,
        FieldStatisticsSelectionProof::AllRecords,
        "both physical root paths were read"
    );
    let flat = numeric(&report, 101)?;
    let nested = numeric(&report, 11)?;
    assert_ne!(
        flat.ordinal, nested.ordinal,
        "literal slash field names must not alias actual nested field paths"
    );
    assert!(
        flat.minimum_uses
            .iter()
            .all(|site| !nested.extrema.source_uses_refs().contains(site)),
        "the actual flat scalar must not enter the nested extrema"
    );
    assert!(
        nested
            .minimum_uses
            .iter()
            .all(|site| !flat.extrema.source_uses_refs().contains(site)),
        "the selected nested value must not enter the flat scalar proof"
    );
    for row in [flat, nested] {
        check_comparison(&report, row)?;
    }
    Ok(())
}

fn check_comparison(
    report: &FieldStatisticsReport<'_>,
    row: &FieldStatisticsRowProof,
) -> TestResult {
    let [min] = row.minimum_uses.as_slice() else {
        return Err("each selected minimum needs one original operand".into());
    };
    let [max] = row.maximum_uses.as_slice() else {
        return Err("each selected maximum needs one original operand".into());
    };
    assert_ne!(min, max, "separate physical inputs win the extrema");
    for site in [min, max] {
        assert!(
            row.extrema.source_uses_refs().contains(site)
                && row.distinct.source_uses_refs().contains(site)
                && !row.empty.source_uses_refs().contains(site)
                && !row.structure.source_uses_refs().contains(site),
            "only numerical and fingerprint comparisons consult these numeric bytes"
        );
        assert_eq!(
            describe(report, site)?.role(),
            SourceUseRole::Payload,
            "the retained selector resolves to the actual numeric payload"
        );
    }
    Ok(())
}

#[test]
fn nonempty_pointer_words_prove_the_empty_count() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Target").field("value", TestType::Int32))
        .structure(
            TestStruct::new("Owner").field("pointer", TestType::StrongPointer("Target".into())),
        )
        .record(TestRecord::new(1, "Owner", "A", "libs/one.xml").value(
            "pointer",
            TestValue::Struct(TestInstance::new("Target").value("value", TestValue::Int32(5))),
        ))
        .record(TestRecord::new(2, "Owner", "B", "libs/two.xml").value("pointer", TestValue::Null))
        .build();
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let row = report
        .reduction
        .statistics()
        .fields()
        .iter()
        .position(|field| field.path() == "pointer")
        .ok_or("the original pointer field exists")?;
    let proof = report.fields.get(row).ok_or("the pointer proof exists")?;
    assert_eq!(
        proof
            .empty
            .source_uses_refs()
            .iter()
            .filter(|site| {
                describe(&report, site).is_ok_and(|row| {
                    row.role() == SourceUseRole::Payload
                        && matches!(
                            row.location(),
                            SourceLocation::Subresource { namespace, field, .. }
                                if namespace == "dataforgeDecodedBytes" && field == "slot"
                        )
                })
            })
            .count(),
        2,
        "both the nonempty pointer and the null pointer determined the empty count"
    );
    Ok(())
}

#[test]
fn selection_retains_where_operands_from_admitted_and_rejected_candidates() -> TestResult {
    let bytes = fixture(false);
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Owner where choice > 0")?;
    let all = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let selected = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        selected.selection_proof,
        FieldStatisticsSelectionProof::QueryRuntime {
            reference_coverage: ReferenceCoverage::Complete,
        },
        "selection proof class records compiled and runtime Query observations"
    );
    assert_eq!(
        selected.reduction.statistics().records(),
        1,
        "one physical candidate is rejected by WHERE"
    );
    assert!(
        selected.selection_origin.source_set_complete(),
        "the original numeric comparison and candidate census prove selection"
    );
    let unfiltered = numeric(&all, 0)?;
    let nested = numeric(&selected, 11)?;
    for site in unfiltered
        .minimum_uses
        .iter()
        .chain(&unfiltered.maximum_uses)
    {
        let operand = describe(&all, site)?;
        assert!(
            selected.inventory.iter_uses().any(|row| {
                row.role() == SourceUseRole::Selector
                    && row.location() == operand.location()
                    && selected
                        .selection_origin
                        .source_uses_refs()
                        .contains(row.id())
                    && nested.extrema.source_uses_refs().contains(row.id())
                    && !nested.minimum_uses.contains(row.id())
            }),
            "rejected and accepted guard values determine the cohort without winning comparisons"
        );
    }
    assert_eq!(
        nested.minimum_uses, nested.maximum_uses,
        "the surviving record is the sole numeric operand"
    );
    assert!(
        nested.extrema.source_set_complete(),
        "the surviving field has a complete direct selection and metric basis"
    );
    for winner in &nested.minimum_uses {
        assert_eq!(
            describe(&selected, winner)?.role(),
            SourceUseRole::Payload,
            "winning numeric operands are original payload reads"
        );
    }
    Ok(())
}

#[test]
fn predicate_uses_absent_field_layout_without_a_runtime_operand() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("other", TestType::Int32))
        .structure(TestStruct::new("NoChoice").parent("Base"))
        .structure(TestStruct::new("Choice").field("chosen", TestType::Int32))
        .record(TestRecord::new(1, "NoChoice", "A", "libs/no-choice.xml"))
        .record(
            TestRecord::new(2, "Choice", "B", "libs/choice.xml")
                .value("chosen", TestValue::Int32(1)),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse(r#"at "libs/*" where chosen > 0"#)?;
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert_eq!(
        report.reduction.statistics().records(),
        1,
        "the first original Record has no compatible WHERE member"
    );
    let proof = report.selection_origin.source_uses_refs();
    for (index, field) in [(0, "firstProperty"), (1, "propertyCount"), (1, "parent")] {
        assert!(
            has_declaration(&report, proof, index, field),
            "absence uses both actual Struct rows and the original inherited property range"
        );
    }
    Ok(())
}

#[test]
fn field_census_keeps_layouts_that_have_no_matching_descendant() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Present").field("value", TestType::Int32))
        .structure(TestStruct::new("Absent").field("marker", TestType::Bool))
        .structure(TestStruct::new("Without").field("group", TestType::Class("Absent".into())))
        .structure(TestStruct::new("With").field("group", TestType::Class("Present".into())))
        .record(TestRecord::new(1, "Without", "A", "libs/without.xml"))
        .record(TestRecord::new(2, "With", "B", "libs/with.xml").value(
            "group",
            TestValue::Struct(TestInstance::new("Present").value("value", TestValue::Int32(12))),
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let row = numeric(&report, 12)?;
    assert!(
        has_declaration(
            &report,
            row.structure.source_uses_refs(),
            1,
            "propertyCount"
        ),
        "the reached sibling layout proves it had no matching descendant"
    );
    assert!(
        has_declaration(
            &report,
            row.structure.source_uses_refs(),
            2,
            "propertyCount"
        ),
        "the admitted root layout establishes the absence of a matching path"
    );
    Ok(())
}

fn has_declaration(
    report: &FieldStatisticsReport<'_>,
    references: &[SourceUseRef],
    index: u64,
    name: &str,
) -> bool {
    references.iter().any(|site| {
        describe(report, site).is_ok_and(|source| {
            source.role() == SourceUseRole::Selector
                && matches!(
                    source.location(),
                    SourceLocation::Subresource { namespace, indices, field }
                        if namespace == "dataforgeStructDefinition"
                            && indices.as_slice() == [index] && field == name
                )
        })
    })
}

#[test]
fn failed_root_read_is_not_a_contributor_to_a_proved_field() -> TestResult {
    let bytes = fixture(true);
    let forge = DataForge::open(&bytes)?;
    let report = FieldStatisticsReport::all(&forge, SourceInput::Unknown)?;
    let [attempt] = report.root_attempts.as_slice() else {
        return Err("one admitted root read must fail at its physical instance".into());
    };
    let description = describe(&report, attempt)?;
    assert_eq!(
        description.role(),
        SourceUseRole::Attempted,
        "failed instance read"
    );
    assert!(
        matches!(
            description.location(),
            SourceLocation::DataForge { record_index: 1, field } if field == "instance"
        ),
        "the exact contributing Record header contains the unresolved instance"
    );
    let nested = numeric(&report, 11)?;
    assert!(
        !nested.extrema.source_set_complete()
            && !nested.extrema.source_uses_refs().contains(attempt),
        "an attempt leaves extrema incomplete without becoming an operand"
    );
    Ok(())
}

fn numeric<'report>(
    report: &'report FieldStatisticsReport<'_>,
    value: i128,
) -> Result<&'report FieldStatisticsRowProof, Box<dyn Error>> {
    let ordinal = report
        .reduction
        .statistics()
        .fields()
        .iter()
        .position(|field| field.min() == Some(Number::Integer(value)))
        .ok_or("the measured numeric row must exist")?;
    Ok(report
        .fields
        .get(ordinal)
        .ok_or("the original row proof must exist")?)
}

fn describe<'report>(
    report: &'report FieldStatisticsReport<'_>,
    source: &SourceUseRef,
) -> Result<&'report SourceUseDescription, Box<dyn Error>> {
    Ok(report
        .inventory
        .iter_uses()
        .find(|site| site.id() == source)
        .ok_or("the reported source-use reference must resolve")?)
}

fn fixture(damaged: bool) -> Vec<u8> {
    let mut second = owner(2, "libs/two.xml", 201, 21, 0);
    if damaged {
        second = second.with_instance_index(u16::MAX);
    }
    DataForgeBuilder::new()
        .structure(TestStruct::new("Group").field("value", TestType::Int32))
        .structure(
            TestStruct::new("Owner")
                .field("group/value", TestType::Int32)
                .field("group", TestType::Class("Group".to_owned()))
                .field("choice", TestType::Int32),
        )
        .record(owner(1, "libs/one.xml", 101, 11, 1))
        .record(second)
        .build()
}

fn owner(id: u128, path: &str, flat: i32, nested: i32, choice: i32) -> TestRecord {
    TestRecord::new(id, "Owner", "Fixture", path)
        .value("group/value", TestValue::Int32(flat))
        .value(
            "group",
            TestValue::Struct(TestInstance::new("Group").value("value", TestValue::Int32(nested))),
        )
        .value("choice", TestValue::Int32(choice))
}
