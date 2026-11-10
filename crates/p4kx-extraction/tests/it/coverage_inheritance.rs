// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, CoverageReport, DataForgeEntry, ReferenceList};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

fn database() -> Vec<u8> {
    let mut builder = DataForgeBuilder::new();
    for family in ["R", "C"] {
        builder = builder
            .structure(TestStruct::new(format!("{family}000")).field("mark", TestType::Int32));
        for index in 1..70 {
            builder = builder.structure(
                TestStruct::new(format!("{family}{index:03}"))
                    .parent(format!("{family}{:03}", index - 1)),
            );
        }
    }
    builder
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", TestType::StrongPointer("C000".to_owned())),
        )
        .record(
            TestRecord::new(1, "R069", "Record", "record.xml").value("mark", TestValue::Int32(7)),
        )
        .record(
            TestRecord::new(2, "EntityClassDefinition", "Entity", "entity.xml").value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new("C069").value("mark", TestValue::Int32(9)),
                )]),
            ),
        )
        .build()
}

fn report() -> Result<CoverageReport, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    assert_eq!(forge.record_count(), 2, "actual stored Records");
    for record in forge.records() {
        assert_eq!(record.tree()?.error_count(), 0, "complete valid source");
    }
    let list = ReferenceList::parse("test record R000\ntest component C000\n")?;
    Ok(CoverageReport::of(&archive, &forge, &list))
}

#[test]
fn distant_record_ancestor_keeps_its_actual_count_and_sample() -> TestResult {
    let report = report()?;
    let item = report.types.first().ok_or("record coverage")?;
    assert!(item.defined, "the original root definition exists");
    assert_eq!(
        (item.records, item.unreadable_records, item.error_values),
        (1, 0, 0),
        "the distant root counts one valid original Record"
    );
    let sample = item.sample.as_ref().ok_or("original sample")?;
    assert_eq!(
        sample.record_path, "record",
        "sample is the actual source Record"
    );
    assert!(
        sample.error.is_none() && sample.json_bytes > 0 && sample.xml_bytes > 0,
        "actual sample writers complete"
    );
    Ok(())
}

#[test]
fn distant_component_ancestor_counts_each_actual_instance() -> TestResult {
    let report = report()?;
    let component = report.components.first().ok_or("component coverage")?;
    assert_eq!(report.unreadable_entities, 0, "valid original entity");
    assert!(
        component.defined && component.reachable,
        "actual Query reaches source"
    );
    assert_eq!(
        (component.entities, component.instances),
        (1, 1),
        "one real entity and one component instance"
    );
    Ok(())
}
