// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue};

pub(super) const OWNER: u128 = 0x1063_0010;
pub(super) const MODE: u128 = 0x1063_0011;
pub(super) const SEAT: &str = "SCItemSeatParams";
pub(super) const EJECTION: &str = "SCItemSeatEjectParams";
pub(super) const ATTACHMENT: &str = "SCSeatActorAttachmentParams";

const ANGLES: &str = "Deg3";
const INTERACTION: &str = "SSharedInteractionParams";
const OPERATOR_MODE: &str = "OperatorModeAvailabilityParams";
const SEAT_TYPE: &str = "SeatTypes";
const ATTACHMENT_TYPE: &str = "SCSeatActorAttachmentType";
const STANCE: &str = "ActorStateFilterByStanceState";

pub(super) fn database(offset_z: f32) -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    builder
        .enumeration(SEAT_TYPE, ["FixtureCrew", "FixtureOperator"])
        .enumeration(ATTACHMENT_TYPE, ["FixtureAttach"])
        .enumeration(STANCE, ["FixtureStance"])
        .structure(seat_definition())
        .structure(ejection_definition())
        .structure(
            TestStruct::new(ATTACHMENT)
                .field("type", TestType::Enum(ATTACHMENT_TYPE.to_owned()))
                .field("boneName", TestType::String)
                .field("stance", TestType::Enum(STANCE.to_owned())),
        )
        .structure(
            TestStruct::new(ANGLES)
                .field("x", TestType::Single)
                .field("y", TestType::Single)
                .field("z", TestType::Single),
        )
        .structure(TestStruct::new(INTERACTION).field("fixtureMarker", TestType::Bool))
        .structure(TestStruct::new(OPERATOR_MODE))
        .record(
            TestEntity::new(
                OWNER,
                "FixtureSeatOwner",
                "libs/foundry/records/entities/scitem/ships/fixture/facility_seats.xml",
            )
            .attach_type("Misc")
            .component(TestValue::Struct(seat("FixtureCrew", Some(offset_z))))
            .component(TestValue::Struct(seat("FixtureOperator", None)))
            .record(),
        )
}

fn seat_definition() -> TestStruct {
    TestStruct::new(SEAT)
        .parent("DataForgeComponentParams")
        .field(
            "availableOperatorModes",
            TestType::ReferenceTo(OPERATOR_MODE.to_owned()),
        )
        .field(
            "fixtureInteraction",
            TestType::StrongPointer(INTERACTION.to_owned()),
        )
        .field("ejection", TestType::StrongPointer(EJECTION.to_owned()))
        .field("seatType", TestType::Enum(SEAT_TYPE.to_owned()))
        .field("minYaw", TestType::Single)
        .field("maxYaw", TestType::Single)
        .field("minPitch", TestType::Single)
        .field("maxPitch", TestType::Single)
        .field("setYawPitchLimits", TestType::Bool)
        .field("actorAttachment", TestType::Class(ATTACHMENT.to_owned()))
        .field("userAnimationDatabase", TestType::String)
        .field("fixtureSeatFact", TestType::UInt16)
}

fn ejection_definition() -> TestStruct {
    TestStruct::new(EJECTION)
        .field("maxLinearVelocity", TestType::Single)
        .field("maxLinearAcceleration", TestType::Single)
        .field("maxAngularVelocity", TestType::Single)
        .field("maxAngularAcceleration", TestType::Single)
        .field("ejectionLoopTime", TestType::Single)
        .field(
            "ejectionInteraction",
            TestType::WeakPointer(INTERACTION.to_owned()),
        )
        .field("offsetAngles", TestType::Class(ANGLES.to_owned()))
}

fn seat(kind: &str, offset_z: Option<f32>) -> TestInstance {
    let interaction = TestInstance::new(INTERACTION);
    let (interaction, ejection, mode) = if let Some(offset_z) = offset_z {
        (
            interaction.named("fixture-interaction"),
            TestValue::Struct(ejection_values(offset_z)),
            TestValue::Reference(MODE),
        )
    } else {
        (interaction, TestValue::Null, TestValue::Null)
    };
    TestInstance::new(SEAT)
        .value("availableOperatorModes", mode)
        .value("fixtureInteraction", TestValue::Struct(interaction))
        .value("ejection", ejection)
        .value("seatType", TestValue::text(kind))
        .value("minYaw", TestValue::Single(-0.0))
        .value("maxYaw", TestValue::Single(120.0))
        .value("minPitch", TestValue::Single(-34.0))
        .value("maxPitch", TestValue::Single(34.0))
        .value("setYawPitchLimits", TestValue::Bool(false))
        .value(
            "actorAttachment",
            TestValue::Struct(
                TestInstance::new(ATTACHMENT)
                    .value("type", TestValue::text("FixtureAttach"))
                    .value("boneName", TestValue::text("fixture_bone"))
                    .value("stance", TestValue::text("FixtureStance")),
            ),
        )
        .value("userAnimationDatabase", TestValue::text("fixture/seat.adb"))
        .value("fixtureSeatFact", TestValue::UInt16(43))
}

fn ejection_values(offset_z: f32) -> TestInstance {
    TestInstance::new(EJECTION)
        .value("maxLinearVelocity", TestValue::Single(10.0))
        .value("maxLinearAcceleration", TestValue::Single(-0.0))
        .value("maxAngularVelocity", TestValue::Single(2.0))
        .value("maxAngularAcceleration", TestValue::Single(3.0))
        .value("ejectionLoopTime", TestValue::Single(0.0))
        .value(
            "ejectionInteraction",
            TestValue::pointer("fixture-interaction"),
        )
        .value(
            "offsetAngles",
            TestValue::Struct(
                TestInstance::new(ANGLES)
                    .value("x", TestValue::Single(5.0))
                    .value("y", TestValue::Single(-0.0))
                    .value("z", TestValue::Single(offset_z)),
            ),
        )
}

pub(super) fn database_with_alternative_ejection(text: Option<&str>) -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    let mut declaration = TestStruct::new(SEAT).parent("DataForgeComponentParams");
    let mut seat = TestInstance::new(SEAT);
    if let Some(text) = text {
        declaration = declaration.field("ejection", TestType::String);
        seat = seat.value("ejection", TestValue::text(text));
    }
    builder.structure(declaration).record(
        TestEntity::new(
            OWNER,
            "FixtureAlternateSeat",
            "libs/foundry/records/entities/scitem/ships/fixture/facility_alternate_seat.xml",
        )
        .attach_type("Misc")
        .component(TestValue::Struct(seat))
        .record(),
    )
}
