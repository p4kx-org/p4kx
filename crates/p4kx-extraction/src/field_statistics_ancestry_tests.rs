// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Mutex;

use p4kx_dataforge::query::{Query, QuerySource};
use p4kx_dataforge::{DataForge, LayoutSource};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::{SourceInput, SourceLocation, SourceUseRole};

use super::FieldStatisticsReport;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn typed_where_keeps_compile_names_and_rejected_runtime_parents() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Station where items[Special].ready = true")?;
    let ObservedSelection {
        layouts: layout,
        runtime: sources,
        records,
    } = observed(&forge, &query)?;
    assert_eq!(
        records,
        query.compile(&forge)?.run()?.indexes(),
        "the observed walk preserves normal typed-array admission"
    );
    assert_qualifier(&layout);
    assert_parent(&sources, 2, 0);
    assert_parent(&sources, 3, 1);
    assert_eq!(
        sources
            .iter()
            .filter(|source| matches!(source, QuerySource::AncestorGap { .. }))
            .count(),
        0,
        "a rejected type with an authored root sentinel has complete ancestry"
    );
    let report = FieldStatisticsReport::query(&forge, SourceInput::Unknown, &query)?;
    assert!(
        report.inventory.iter_uses().any(|usage| {
            usage.role() == SourceUseRole::Selector
                && report
                    .selection_origin
                    .source_uses_refs()
                    .contains(usage.id())
                && matches!(
                    usage.location(),
                    SourceLocation::Subresource {namespace, indices, field}
                        if namespace == "dataforgeStructDefinition"
                            && indices.as_slice() == [2] && field == "parent"
                )
        }),
        "the rejected original element's parent is part of WHERE selection evidence"
    );
    Ok(())
}

fn assert_qualifier(layout: &[LayoutSource]) {
    assert!(
        layout.iter().any(|source| matches!(
            source,
            LayoutSource::TypeName {
                index: 1,
                name: Some(_),
                ..
            }
        )),
        "the selected type name uses its own original text span"
    );
    assert!(
        layout.iter().any(|source| matches!(
            source,
            LayoutSource::TypeParent {
                index: 1,
                stored_parent: 0,
                ..
            }
        )),
        "the typed qualifier must derive from the original declared element type"
    );
    assert_eq!(
        layout
            .iter()
            .filter(|source| matches!(source, LayoutSource::TypeName { index: 5, .. }))
            .count(),
        0,
        "the name lookup ends at its actual accepted duplicate or first candidate"
    );
}

#[test]
fn followed_reference_binds_the_target_ancestry() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Station where linked->ready = true")?;
    let ObservedSelection {
        runtime: sources,
        records,
        ..
    } = observed(&forge, &query)?;
    assert_eq!(
        records,
        [0],
        "the derived Record supplies the compared value"
    );
    assert_parent(&sources, 3, 1);
    assert_parent(&sources, 1, 0);
    Ok(())
}

#[test]
fn select_only_qualifier_does_not_enter_where_selection_proof() -> TestResult {
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Station select items[Special].ready")?;
    let ObservedSelection {
        layouts: layout,
        runtime: sources,
        records,
    } = observed(&forge, &query)?;
    assert_eq!(
        records,
        [0],
        "ordinary Record selection does not evaluate SELECT"
    );
    assert_eq!(
        layout.len(),
        0,
        "SELECT layout validation is not a WHERE selection input"
    );
    assert_eq!(
        sources.len(),
        0,
        "SELECT also does not contribute runtime predicate operands"
    );
    Ok(())
}

struct ObservedSelection {
    layouts: Vec<LayoutSource>,
    runtime: Vec<QuerySource>,
    records: Vec<u32>,
}

fn observed(forge: &DataForge<'_>, query: &Query) -> Result<ObservedSelection, Box<dyn Error>> {
    let declarations = Mutex::new(Vec::new());
    let sources = Mutex::new(Vec::new());
    let (compiled, _) = query.compile_observing_predicate_layouts(
        forge,
        |_| {},
        |source| {
            declarations
                .lock()
                .expect("the test owns the declaration observer")
                .push(source);
        },
        |_| {},
    )?;
    let run = compiled
        .run_observing_records(
            || true,
            |source| {
                sources
                    .lock()
                    .expect("the test owns the runtime observer")
                    .push(source);
            },
            |_| {},
        )?
        .ok_or("the supplied predicate walk was not cancelled")?;
    Ok(ObservedSelection {
        layouts: declarations.into_inner()?,
        runtime: sources.into_inner()?,
        records: run.indexes().to_vec(),
    })
}

fn assert_parent(sources: &[QuerySource], index: usize, stored_parent: u32) {
    assert!(
        sources.contains(&QuerySource::AncestorParent {
            index,
            stored_parent
        }),
        "the actual original parent word of type {index} was compared"
    );
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Part").field("ready", TestType::Bool))
        .structure(TestStruct::new("Special").parent("Part"))
        .structure(TestStruct::new("Skipped").parent("Part"))
        .structure(TestStruct::new("Accepted").parent("Special"))
        .structure(
            TestStruct::new("Station")
                .array("items", TestType::StrongPointer("Part".into()))
                .field("linked", TestType::ReferenceTo("Part".into())),
        )
        .structure(TestStruct::new("Unvisited"))
        .record(
            TestRecord::new(1, "Station", "Station", "synthetic/station.xml")
                .value(
                    "items",
                    TestValue::Array(vec![
                        TestValue::Struct(TestInstance::new("Skipped")),
                        TestValue::Struct(
                            TestInstance::new("Accepted").value("ready", TestValue::Bool(true)),
                        ),
                    ]),
                )
                .value("linked", TestValue::Reference(99)),
        )
        .record(
            TestRecord::new(99, "Accepted", "Anchor", "synthetic/anchor.xml")
                .value("ready", TestValue::Bool(true)),
        )
        .build()
}
