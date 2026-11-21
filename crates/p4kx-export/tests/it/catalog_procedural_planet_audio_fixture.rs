// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const SOURCE_TYPE: &str = "ProceduralPlanetAudioData";
pub(super) const EXISTING: &str = "firstRoomIsPlanetRoomRtpc";
pub(super) const ADDED: &str = "firstRoomIsPlanetRoomNotIncludingShipsRtpc";
pub(super) const OTHER: &str = "firstRoomIsPlanetRoomOrVehicleRoomRtpc";
const NEIGHBOR: &str = "SyntheticPlanetAudioNeighbor";

#[derive(Clone, Copy)]
pub(super) enum Layout {
    Extended,
    Complete,
    Unreadable,
}

pub(super) fn database(layout: Layout) -> Vec<u8> {
    let class = || TestType::Class("AudioRtpc".to_owned());
    let mut definition = TestStruct::new(SOURCE_TYPE)
        .field(EXISTING, class())
        .array("entries", TestType::String)
        .array("disturbanceLists", TestType::Reference)
        .array("effectTagMovementRtpcs", TestType::UInt32)
        .field("futureRoot", TestType::Double);
    let existing = match layout {
        Layout::Unreadable => unreadable(),
        Layout::Extended | Layout::Complete => TestValue::text("synthetic_existing"),
    };
    let mut record = TestRecord::new(
        0x1149,
        SOURCE_TYPE,
        "ProceduralPlanetAudioData.Test",
        "synthetic/dev/planet.xml",
    )
    .value(EXISTING, rtpc(existing, 1.25))
    .value("entries", TestValue::Array(vec![TestValue::text("tone")]))
    .value("disturbanceLists", TestValue::Array(vec![TestValue::Null]))
    .value("effectTagMovementRtpcs", TestValue::Array(Vec::new()))
    .value("futureRoot", TestValue::Double(1.000_000_000_25));
    if matches!(layout, Layout::Complete | Layout::Extended) {
        definition = definition.field(ADDED, class());
        record = record.value(ADDED, rtpc(TestValue::text(""), 2.5));
    }
    if matches!(layout, Layout::Extended) {
        definition = definition.field(OTHER, TestType::Class("SyntheticOtherClass".to_owned()));
        record = record.value(
            OTHER,
            TestValue::Struct(TestInstance::new("SyntheticOtherClass")),
        );
    }
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(definition)
        .structure(
            TestStruct::new("AudioRtpc")
                .field("rtpc", TestType::String)
                .field("extraScale", TestType::Single),
        )
        .structure(TestStruct::new("SyntheticOtherClass"))
        .record(record);
    if matches!(layout, Layout::Complete) {
        builder = builder
            .structure(TestStruct::new(NEIGHBOR).field(EXISTING, class()))
            .record(
                TestRecord::new(
                    0x1150,
                    NEIGHBOR,
                    "SyntheticPlanetAudioNeighbor.Test",
                    "synthetic/dev/neighbor.xml",
                )
                .value(EXISTING, rtpc(unreadable(), 4.0)),
            );
    }
    builder.build()
}

fn rtpc(name: TestValue, extra_scale: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("AudioRtpc")
            .value("rtpc", name)
            .value("extraScale", TestValue::Single(extra_scale)),
    )
}

fn unreadable() -> TestValue {
    TestValue::Raw(u32::MAX.to_le_bytes().to_vec())
}
