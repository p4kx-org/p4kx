// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestChunkFile,
    TestContainer, TestElement, TestEntity, TestEntry, TestInstance, TestObjectContainer,
    TestRecord, TestStruct, TestType, TestValue,
};

use crate::commodity_v3_fixture::{self as common, TestResult};

pub(super) const SHIP: u128 = 0x1063_0520;
pub(super) const SEAT: u128 = 0x1063_0521;
pub(super) const ROOM: &str = r"ObjectContainers\Ships\Fixture\interior.socpak";
pub(super) const DESCRIPTOR: &str = "interior.xml";
pub(super) const STREAM: &str = "interior_room.soc";
pub(super) const CHILD: &str = "fixture_child";
pub(super) const CHILD_STREAM: &str = "child_room.soc";

#[derive(Clone, Copy)]
pub(super) enum Case {
    Direct,
    RepeatedStream,
    NestedChildren,
    ChildFaults,
    MissingStreamAndChild,
    AmbiguousStream,
    AmbiguousClass,
    LossyDescriptor,
    UnexpectedDescriptorRoot,
    LossyChunk,
}

pub(super) fn archive(case: Case) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture_soc_facilities.p4k");
    let descriptor = descriptor(case)?;
    let mut room = TestContainer::new()
        .entry(TestEntry::stored(DESCRIPTOR, descriptor))
        .entry(TestEntry::stored(STREAM, soc(case)?))
        .entry(TestEntry::stored("unselected.soc", b"not a named stream"));
    if matches!(case, Case::AmbiguousStream) {
        room = room.entry(TestEntry::stored(STREAM.to_ascii_uppercase(), soc(case)?));
    }
    if matches!(case, Case::NestedChildren | Case::ChildFaults) {
        let child = TestObjectContainer::new(CHILD).stream(CHILD_STREAM);
        let child = if matches!(case, Case::NestedChildren) {
            child.child(ROOM, [0.0; 3], [1.0, 0.0, 0.0, 0.0])
        } else {
            child
        }
        .cryxml();
        let pak = TestContainer::new()
            .entry(TestEntry::stored(format!("{CHILD}.xml"), child))
            .entry(TestEntry::stored(CHILD_STREAM, soc(case)?))
            .build();
        room = room.entry(TestEntry::stored(format!("{CHILD}.socpak"), pak));
    }
    if matches!(case, Case::ChildFaults) {
        room = room
            .entry(TestEntry::stored("ambiguous.socpak", b"first entry"))
            .entry(TestEntry::stored("AMBIGUOUS.SOCPAK", b"second entry"));
    }
    let archive = ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database(case).build()))
        .entry(TestEntry::stored(format!(r"Data\{ROOM}"), room.build()).encrypted())
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ));
    let archive = if matches!(case, Case::NestedChildren) {
        archive.entry(TestEntry::stored(
            format!("{CHILD}.socpak"),
            b"unselected outer decoy",
        ))
    } else {
        archive
    };
    archive.write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn descriptor(case: Case) -> TestResult<Vec<u8>> {
    let mut descriptor = if matches!(case, Case::UnexpectedDescriptorRoot) {
        format!("<Unrelated><StreamingOCs><OC name='{STREAM}'/></StreamingOCs></Unrelated>")
            .into_bytes()
    } else {
        let room = TestObjectContainer::new("interior").stream(STREAM);
        match case {
            Case::RepeatedStream => room.stream(STREAM),
            Case::NestedChildren => room
                .child(CHILD, [2.0, 3.0, 4.0], [0.0, 0.0, 0.0, 1.0])
                .child(CHILD, [10.0, 3.0, 4.0], [0.0, 0.0, 0.0, 1.0]),
            Case::ChildFaults => room
                .raw_child(&[("name", CHILD), ("pos", "bad,3,4"), ("rot", "0,0,0,1")])
                .raw_child(&[("name", "ambiguous")])
                .raw_child(&[("name", "../skip")])
                .raw_child(&[("name", "first"), ("NAME", "second")]),
            Case::MissingStreamAndChild => {
                room.stream("missing.soc")
                    .child("missing_child", [0.0; 3], [1.0, 0.0, 0.0, 0.0])
            }
            _ => room,
        }
        .cryxml()
    };
    if matches!(case, Case::LossyDescriptor) {
        break_string(&mut descriptor, STREAM.as_bytes())?;
    }
    Ok(descriptor)
}

fn database(case: Case) -> DataForgeBuilder {
    const COMPONENT: &str = "SCItemSeatParams";
    const SEAT_TYPE: &str = "SeatTypes";
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .enumeration(SEAT_TYPE, ["FixtureSeat"])
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field("seatType", TestType::Enum(SEAT_TYPE.to_owned())),
        )
        .record(
            TestEntity::new(
                SHIP,
                "FixtureInteriorHost",
                "libs/foundry/records/entities/spaceships/fixture_interior_host.xml",
            )
            .vehicle("", "")
            .object_container(ROOM, "fixture_room_bone", [10.0, 20.0, 30.0], [0.0; 3])
            .record(),
        )
        .record(
            TestEntity::new(
                SEAT,
                "FixtureInteriorSeat",
                "libs/foundry/records/entities/scitem/fixture/interior_seat.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(
                TestInstance::new(COMPONENT).value("seatType", TestValue::text("FixtureSeat")),
            ))
            .record(),
        );
    if matches!(case, Case::AmbiguousClass) {
        return database
            .structure(TestStruct::new("FixtureUnselectedRecord"))
            .record(TestRecord::new(
                SEAT,
                "FixtureUnselectedRecord",
                "FixtureUnselectedRecord.Duplicate",
                "libs/foundry/records/fixture/duplicate.xml",
            ));
    }
    database
}

fn soc(case: Case) -> TestResult<Vec<u8>> {
    let class = common::guid(SEAT);
    let entity = |name: &str, position: &str| {
        TestElement::new("Entity")
            .attribute("Name", name)
            .attribute("EntityClass", "EntityComponent")
            .attribute("EntityClassGUID", &class)
            .attribute("Pos", position)
            .attribute("Rotate", "1,0,0,0")
            .attribute("Scale", "1,1,1")
    };
    let mut xml = TestElement::new("Entities")
        .child(entity("seat_valid", "1,2,3").child(geometry()))
        .child(TestElement::new("Metadata"))
        .child(entity("seat_invalid", "bad,0,0"))
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    if matches!(case, Case::LossyChunk) {
        break_string(&mut xml, b"seat_valid")?;
    }
    Ok(TestChunkFile::crch()
        .chunk(0x0001, 5, vec![0; 4])
        .chunk(0x0004, 3, xml)
        .bytes())
}

fn break_string(bytes: &mut [u8], original: &[u8]) -> TestResult {
    let offset = bytes
        .windows(original.len())
        .position(|field| field == original)
        .ok_or("the encoded fixture string")?;
    *bytes.get_mut(offset).ok_or("the original fixture byte")? = 0xff;
    Ok(())
}

fn geometry() -> TestElement {
    TestElement::new("PropertiesDataCore").child(TestElement::new("EntityGeometryResource").child(
        TestElement::new("Geometry").child(
            TestElement::new("Geometry").child(
                TestElement::new("Geometry").attribute("path", "objects/fixture/unread.cgf"),
            ),
        ),
    ))
}
