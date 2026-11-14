// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Reference, TagTree, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::Reader;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const OWNER: u128 = 0x6d00_0000_0000_4000_8000_0000_0000_0001;

fn reference<'t, 'f>(
    value: Option<&'t Value<'f>>,
) -> Result<&'t Reference<'f>, Box<dyn std::error::Error>> {
    match value {
        Some(Value::Reference(target)) => Ok(target),
        _ => Err("reference missing".into()),
    }
}

#[test]
fn weak_locations_never_cross_physical_owners_with_the_same_guid_and_path() -> TestResult {
    let record = |name, child| {
        TestRecord::new(OWNER, "Root", name, "synthetic/alias.xml")
            .value(
                "owned",
                TestValue::Struct(TestInstance::new("Child").named(child)),
            )
            .value("link", TestValue::pointer(child))
    };
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Child"))
        .structure(
            TestStruct::new("Root")
                .field("owned", TestType::StrongPointer("Child".to_owned()))
                .field("link", TestType::WeakPointer("Child".to_owned())),
        )
        .record(record("Root.First", "first"))
        .record(record("Root.Second", "second"))
        .build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let first = records.first().ok_or("first owner missing")?.tree()?;
    let second = records.get(1).ok_or("second owner missing")?.tree()?;
    let own = reference(first.field("link"))?;
    let foreign = reference(second.field("link"))?;
    let tags = TagTree::default();
    let reader = Reader::for_record(&tags, &first);
    assert_eq!(
        first.guid(),
        second.guid(),
        "the fixture has a real GUID alias"
    );
    assert_eq!(
        first.record_path(),
        second.record_path(),
        "path comparison cannot distinguish the owners"
    );
    assert_eq!(
        own.location(),
        foreign.location(),
        "both owners expose the same relative canonical location"
    );
    assert_eq!(
        own.record_index(),
        0,
        "the first weak reference carries its actual physical owner"
    );
    assert_eq!(
        foreign.record_index(),
        1,
        "the second weak reference retains its distinct physical owner"
    );
    assert!(
        reader.owns(own) && reader.owned_target(own).is_some(),
        "the original physical owner remains resolvable"
    );
    assert!(
        !reader.owns(foreign) && reader.owned_target(foreign).is_none(),
        "a matching GUID and location never authorize following another physical owner"
    );
    Ok(())
}

#[test]
fn normal_guid_and_weak_record_links_report_the_actual_resolved_target_index() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Root"))
        .structure(
            TestStruct::new("Holder")
                .field("normal", TestType::Reference)
                .field("weak", TestType::WeakPointer("Root".to_owned())),
        )
        .record(TestRecord::new(
            OWNER,
            "Root",
            "Root.First",
            "synthetic/alias.xml",
        ))
        .record(TestRecord::new(
            OWNER,
            "Root",
            "Root.Second",
            "synthetic/alias.xml",
        ))
        .record(
            TestRecord::new(
                OWNER + 1,
                "Holder",
                "Holder.Synthetic",
                "synthetic/holder.xml",
            )
            .value("normal", TestValue::Reference(OWNER))
            .value("weak", TestValue::pointer("Root.Second")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let holder = forge.records().nth(2).ok_or("Holder missing")?.tree()?;
    assert_eq!(
        holder.record_index(),
        2,
        "canonical owner identity comes from the actual source table position"
    );
    assert_eq!(
        reference(holder.field("normal"))?.record_index(),
        0,
        "GUID first-wins resolution reports the first actual target"
    );
    assert_eq!(
        reference(holder.field("weak"))?.record_index(),
        1,
        "instance-based weak resolution reports its distinct actual target"
    );
    Ok(())
}
