// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_dataforge::{
    DataForge, PositionedRecord, PositionedReferenceSources, RecordJsonBytes,
    RecordJsonRecordField, RecordJsonSource, TreeBuilder,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

#[path = "record_positioned_links_json.rs"]
mod observed_json;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const STORED_GUID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0];
const DANGLING_GUID: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 77, 0, 0, 0, 0, 0, 0, 0];

fn database() -> Vec<u8> {
    let mut requested_guid = vec![0xaa, 0xbb, 0xcc, 0xdd];
    requested_guid.extend(STORED_GUID);
    let mut dangling_guid = vec![0xdd, 0xcc, 0xbb, 0xaa];
    dangling_guid.extend(DANGLING_GUID);
    let node = TestType::Class("Node".to_owned());
    DataForgeBuilder::new()
        .structure(TestStruct::new("Node").field("weight", TestType::Int32))
        .structure(
            TestStruct::new("Owner")
                .field("reference", TestType::Reference)
                .field("self", TestType::WeakPointer("Owner".to_owned()))
                .field("external", TestType::WeakPointer("Node".to_owned()))
                .field("local", TestType::WeakPointer("Node".to_owned()))
                .array("nodes", node)
                .array("references", TestType::Reference)
                .field("nested", TestType::Class("LinkHolder".to_owned()))
                .field("dangling", TestType::Reference),
        )
        .structure(TestStruct::new("LinkHolder").field("ownedLink", TestType::Reference))
        .record(
            TestRecord::new(1, "Owner", "Owner.One", "owner.xml")
                .value("reference", TestValue::Raw(requested_guid))
                .value("self", TestValue::pointer("Owner.One"))
                .value("external", TestValue::pointer("Node.Decoy"))
                .value(
                    "nodes",
                    TestValue::Array(vec![
                        TestValue::Struct(TestInstance::new("Node").named("first")),
                        TestValue::Struct(TestInstance::new("Node").named("second")),
                    ]),
                )
                .value("local", TestValue::pointer("second"))
                .value(
                    "references",
                    TestValue::Array(vec![TestValue::Reference(2)]),
                )
                .value(
                    "nested",
                    TestValue::Struct(
                        TestInstance::new("LinkHolder").value("ownedLink", TestValue::Reference(2)),
                    ),
                )
                .value("dangling", TestValue::Raw(dangling_guid)),
        )
        .record(TestRecord::new(2, "Node", "Node.Selected", "selected.xml"))
        .record(TestRecord::new(2, "Node", "Node.Decoy", "decoy.xml"))
        .build()
}

fn first_record<'source>(
    input: &'source DataForge<'source>,
) -> TestResult<PositionedRecord<'source>> {
    let record = input.record(0).ok_or("fixture owner Record")?;
    Ok(TreeBuilder::new().build_positioned(&record)?)
}

#[test]
fn positioned_links_distinguish_stored_guid_and_actual_weak_record_targets() -> TestResult {
    let bytes = database();
    let input = DataForge::open(&bytes)?;
    let positioned = first_record(&input)?;
    let fields = positioned.root_fields()?;
    let stored = fields
        .get(0)
        .and_then(|site| site.reference_sources())
        .ok_or("stored Reference")?;
    assert!(stored.has_stored_guid(), "the locator is the stored GUID");
    assert!(
        stored.target_guid().is_none(),
        "GUID evidence never comes from a duplicate target header"
    );
    assert_eq!(
        raw(&bytes, stored.slot(), RecordJsonBytes::Slot)?,
        STORED_GUID,
        "unused caller u32 is excluded"
    );
    expect_record(&stored, 1, None);
    let self_pointer = fields
        .get(1)
        .and_then(|site| site.reference_sources())
        .ok_or("weak self root")?;
    expect_record(&self_pointer, 0, Some(0));
    let external = fields
        .get(2)
        .and_then(|site| site.reference_sources())
        .ok_or("weak external root")?;
    expect_record(&external, 2, Some(2));
    assert_eq!(
        raw(&bytes, external.slot(), RecordJsonBytes::Slot)?.len(),
        8,
        "actual weak pointer words"
    );
    Ok(())
}

#[test]
fn positioned_links_preserve_pool_coordinates_and_local_target_route() -> TestResult {
    let bytes = database();
    let input = DataForge::open(&bytes)?;
    let positioned = first_record(&input)?;
    let fields = positioned.root_fields()?;
    let array = fields.get(5).ok_or("Reference array")?;
    let entry = array
        .element(0)
        .and_then(|site| site.reference_sources())
        .ok_or("Reference pool element")?;
    let slot = raw(&bytes, entry.slot(), RecordJsonBytes::ArrayElement)?;
    assert_eq!(slot.len(), 16, "actual GUID suffix of a pool row");
    assert!(
        entry.caller().sources().iter().any(|source| matches!(
            source,
            RecordJsonSource::Bytes {
                kind: RecordJsonBytes::ArrayHeader,
                ..
            }
        )),
        "actual array admission"
    );
    expect_record(&entry, 1, None);

    let weak = fields
        .get(3)
        .and_then(|site| site.reference_sources())
        .ok_or("weak owned pointer")?;
    assert_eq!(
        weak.target_guid(),
        Some(RecordJsonSource::Record {
            index: 0,
            field: RecordJsonRecordField::Guid
        }),
        "an owned target exports its owning Record GUID"
    );
    assert_eq!(
        weak.target_type().sources(),
        [RecordJsonSource::StructName { index: 0 }],
        "actual Node definition independent of the owner type"
    );
    let path = weak.instance_path().ok_or("local target positions")?;
    assert!(
        path.complete() && weak.target_route().complete(),
        "actual decoder route position"
    );
    assert!(
        path.sources()
            .contains(&RecordJsonSource::PropertyName { index: 5 }),
        "the nodes declaration spells the path"
    );
    assert!(
        path.sources().iter().any(|source| matches!(
            source,
            RecordJsonSource::Bytes {
                kind: RecordJsonBytes::ArrayHeader,
                ..
            }
        )),
        "the target array header admits the selected index"
    );
    Ok(())
}

fn expect_record(witness: &PositionedReferenceSources, row: u32, guid_on_row: Option<u32>) {
    assert_eq!(
        witness.target_guid(),
        guid_on_row.map(|index| RecordJsonSource::Record {
            index,
            field: RecordJsonRecordField::Guid
        }),
        "the exact canonical target row",
    );
    assert!(
        witness.instance_path().is_none(),
        "Record target has no owned path"
    );
    assert!(
        witness.record_path().complete() && witness.target_type().complete(),
        "original target headers were captured"
    );
    assert!(
        witness
            .record_path()
            .sources()
            .contains(&RecordJsonSource::Record {
                index: row,
                field: RecordJsonRecordField::FileName,
            }),
        "actual decoder-admitted file name"
    );
    assert!(
        witness
            .target_type()
            .sources()
            .contains(&RecordJsonSource::Record {
                index: row,
                field: RecordJsonRecordField::Struct,
            }),
        "actual decoder-admitted type selector"
    );
}

fn raw<'archive>(
    data: &'archive [u8],
    slot: Option<RecordJsonSource<'static>>,
    expected: RecordJsonBytes,
) -> TestResult<&'archive [u8]> {
    let Some(RecordJsonSource::Bytes { start, end, kind }) = slot else {
        return Err("raw link storage".into());
    };
    assert_eq!(kind, expected, "actual inline or pool location");
    Ok(data
        .get(usize::try_from(start)?..usize::try_from(end)?)
        .ok_or("source span")?)
}
