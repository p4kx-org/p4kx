// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::query::{Candidate, Query, QuerySchema};
use p4kx_dataforge::{DataForge, DataForgeError, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn database(length: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut builder = DataForgeBuilder::new();
    let mut record = TestRecord::new(1, name(length - 1), "Example", "example.xml");
    for index in 0..length {
        let mut structure = TestStruct::new(name(index))
            .field(format!("f{index:03}"), TestType::Int32)
            .declared_size(u32::try_from((index + 1) * 4)?);
        if index > 0 {
            structure = structure.parent(name(index - 1));
        }
        builder = builder.structure(structure);
        record = record.value(
            format!("f{index:03}"),
            TestValue::Int32(i32::try_from(index)?),
        );
    }
    Ok(builder
        .structure(
            TestStruct::new("Holder").array("parts", TestType::StrongPointer(name(length - 1))),
        )
        .record(record)
        .build())
}

fn name(index: usize) -> String {
    format!("N{index:03}")
}

fn assert_original_chain(forge: &DataForge<'_>, length: usize) -> TestResult {
    assert_eq!(forge.record_count(), 1, "actual stored Record");
    assert_eq!(forge.struct_count(), length + 1, "including Holder");
    let record = forge.records().next().ok_or("Record")?;
    let mut current = Some(record.struct_type()?);
    for index in (0..length).rev() {
        let structure = current.ok_or("missing original ancestor")?;
        assert_eq!(structure.name()?, name(index), "original ancestor order");
        assert_eq!(
            structure.size(),
            u32::try_from((index + 1) * 4)?,
            "declared original inherited size"
        );
        assert_eq!(structure.properties().count(), 1, "one declared property");
        current = structure.parent();
    }
    assert!(current.is_none(), "stored chain terminates without a cycle");
    Ok(())
}

#[test]
fn complete_tree_keeps_each_original_inherited_field() -> TestResult {
    for length in [65, 96] {
        let bytes = database(length)?;
        let forge = DataForge::open(&bytes)?;
        assert_original_chain(&forge, length)?;
        let tree = forge.records().next().ok_or("Record")?.tree()?;
        assert_eq!(tree.fields().len(), length, "every inherited field");
        assert_eq!(tree.error_count(), 0, "valid scalar layout");
        for (index, field) in tree.fields().iter().enumerate() {
            assert_eq!(field.name(), format!("f{index:03}"), "original field order");
            assert_eq!(
                field.value(),
                &Value::Int32(i32::try_from(index)?),
                "original scalar value"
            );
        }
    }
    Ok(())
}

#[test]
fn query_selects_the_record_through_its_original_root_ancestor() -> TestResult {
    let bytes = database(96)?;
    let forge = DataForge::open(&bytes)?;
    assert_original_chain(&forge, 96)?;
    let query = Query::parse("from N000 select f000")?.compile(&forge)?;
    let run = query.run()?;
    assert_eq!(run.indexes(), [0], "actual descendant Record is selected");
    let rows = run.rows(0..1);
    let cells = rows
        .first()
        .ok_or("selected row")?
        .cells()
        .map_err(ToString::to_string)?;
    assert_eq!(
        cells.first().and_then(Option::as_ref),
        Some(&Value::Int32(0))
    );
    Ok(())
}

#[test]
fn completion_keeps_a_distant_ancestor_and_inherited_field() -> TestResult {
    let bytes = database(96)?;
    let forge = DataForge::open(&bytes)?;
    assert_original_chain(&forge, 96)?;
    let schema = QuerySchema::of(&forge)?;
    let text = "from Holder select parts[N000";
    let completion = schema.complete(text, text.len()).ok_or("type completion")?;
    let names: Vec<_> = completion
        .candidates()
        .iter()
        .map(Candidate::name)
        .collect();
    let expected = ["N000"];
    assert_eq!(
        names, expected,
        "the actual distant ancestor remains related"
    );
    let text = "from N095 select f000";
    let completion = schema
        .complete(text, text.len())
        .ok_or("field completion")?;
    assert_eq!(completion.candidates().len(), 1, "exact original field");
    assert_eq!(completion.more(), 0, "no omitted candidates");
    Ok(())
}

#[test]
fn a_cycle_after_many_distinct_ancestors_keeps_its_real_diagnosis() -> TestResult {
    let length = 70;
    let mut builder = DataForgeBuilder::new();
    for index in 0..length {
        let parent = if index + 1 == length { 1 } else { index + 1 };
        builder = builder.structure(
            TestStruct::new(name(index))
                .parent(name(parent))
                .declared_size(0),
        );
    }
    let bytes = builder
        .record(TestRecord::new(1, name(0), "Cycle", "cycle.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;
    assert_eq!(
        forge.struct_count(),
        length,
        "actual distinct cycle definitions"
    );
    let record = forge.records().next().ok_or("Record")?;
    let mut structure = record.struct_type()?;
    for index in 0..length {
        assert_eq!(structure.name()?, name(index), "original distinct ancestor");
        structure = structure.parent().ok_or("original cycle edge")?;
    }
    assert_eq!(structure.name()?, name(1), "actual repeated ancestor");
    assert!(
        matches!(
            record.tree(),
            Err(DataForgeError::StructCycle { struct_index: 0 })
        ),
        "the repeated original index is a real cycle"
    );
    Ok(())
}

#[test]
fn short_chains_keep_the_same_values_and_order() -> TestResult {
    for length in [1, 64] {
        let bytes = database(length)?;
        let forge = DataForge::open(&bytes)?;
        assert_original_chain(&forge, length)?;
        let tree = forge.records().next().ok_or("Record")?.tree()?;
        assert_eq!(tree.fields().len(), length, "all original fields");
        assert_eq!(
            tree.field("f000"),
            Some(&Value::Int32(0)),
            "original root value"
        );
        assert_eq!(tree.error_count(), 0, "valid source remains valid");
    }
    Ok(())
}
