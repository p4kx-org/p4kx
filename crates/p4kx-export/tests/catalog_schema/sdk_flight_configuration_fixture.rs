// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;

pub(super) const CONTROLLER: &str = "SCItemFlightControllerParams";
pub(super) const IFCS: &str = "IFCSParams";
pub(super) const THRUSTER: &str = "SCItemThrusterParams";
const OWNER: &str = "EntityClassDefinition";
const BASE: &str = "FixtureComponent";
const CONTROLLER_BASE: &str = "FixtureFlightControllerBase";
const EFFECT: &str = "FixtureEffect";
const INTERACTION: &str = "FixtureInteraction";
const COLLISION: &str = "FixtureCollision";
const BURN: &str = "FixtureAfterburner";
const CURVE: &str = "FixtureCurve";
const POINT: &str = "FixturePoint";
const ROLE: &str = "FixtureThrusterRole";
pub(super) const RAW_NAN: u32 = 0x7fc0_1234;
pub(super) const UNREADABLE_BODY: u32 = 2;
pub(super) const UNREADABLE_HEADER: u32 = 4;

pub(super) fn archive(malformed_unrelated_owner: bool) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let mut data = DataForgeBuilder::new()
        .version(8)
        .enumeration(ROLE, ["Main", "Maneuver"])
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(OWNER).array("Components", TestType::StrongPointer(BASE.into())))
        .structure(TestStruct::new(EFFECT))
        .structure(TestStruct::new(INTERACTION));
    data = controller_schema(data);
    data = ifcs_schema(data);
    data = data.structure(
        TestStruct::new(THRUSTER)
            .parent(BASE)
            .field("thrusterType", TestType::Enum(ROLE.into()))
            .field("thrustCapacity", TestType::Single)
            .field("thrustCapacityNew", TestType::Single),
    );
    let mut records = records(data, malformed_unrelated_owner);
    if malformed_unrelated_owner {
        records = records.record(owner(
            4,
            "FixtureUnreadableHeader",
            "fixtures/flight/unreadable_header.xml",
            TestValue::Array(vec![ifcs()]),
        ));
    }
    let mut database = records.build();
    if malformed_unrelated_owner {
        corrupt_owner_path(&mut database)?;
    }
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database).encrypted())
        .write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn records(data: DataForgeBuilder, malformed_unrelated_owner: bool) -> DataForgeBuilder {
    data.record(owner(
        1,
        "FixturePrimary",
        "fixtures/flight/primary.xml",
        TestValue::Array(vec![
            controller(),
            TestValue::Null,
            ifcs(),
            thruster("Main", 7.5),
            thruster("Maneuver", f32::from_bits(RAW_NAN)),
        ]),
    ))
    .record(owner(
        1,
        "FixtureAlias",
        "fixtures/flight/alias.xml",
        TestValue::Array(vec![
            thruster("Maneuver", f32::from_bits(RAW_NAN)),
            controller(),
            TestValue::Null,
            ifcs(),
        ]),
    ))
    .record(owner(
        2,
        "FixtureUnrelated",
        "fixtures/flight/unrelated.xml",
        unrelated_components(malformed_unrelated_owner),
    ))
    .record(TestRecord::new(
        3,
        EFFECT,
        "FixtureTag",
        "fixtures/flight/tag.xml",
    ))
}

fn corrupt_owner_path(database: &mut [u8]) -> TestResult<()> {
    let first_path_byte = {
        let forge = DataForge::open(database)?;
        let record = forge
            .record(UNREADABLE_HEADER)
            .ok_or("physical header fixture Record")?;
        let path = record.record_path()?;
        assert_eq!(
            path, "fixtures/flight/unreadable_header",
            "the exact selected owner path defines the corruption location"
        );
        path.as_ptr()
            .addr()
            .checked_sub(database.as_ptr().addr())
            .ok_or("path bytes borrowed from the synthetic DataForge")?
    };
    *database
        .get_mut(first_path_byte)
        .ok_or("the original path byte is inside the synthetic DataForge")? = 0xff;
    Ok(())
}

fn owner(guid: u128, name: &str, path: &str, components: TestValue) -> TestRecord {
    TestRecord::new(guid, OWNER, name, path).value("Components", components)
}

fn unrelated_components(malformed: bool) -> TestValue {
    if malformed {
        TestValue::Raw([1_u32.to_le_bytes(), u32::MAX.to_le_bytes()].concat())
    } else {
        TestValue::Array(vec![
            TestValue::Null,
            TestValue::Struct(TestInstance::new(BASE)),
        ])
    }
}

fn controller_schema(data: DataForgeBuilder) -> DataForgeBuilder {
    data.structure(TestStruct::new(COLLISION).field("collisionWarnSpeed", TestType::Single))
        .structure(
            TestStruct::new(CONTROLLER_BASE)
                .parent(BASE)
                .field("fuelWarningDisplayTime", TestType::Single),
        )
        .structure(
            TestStruct::new(CONTROLLER)
                .parent(CONTROLLER_BASE)
                .field(
                    "disableESPInteraction",
                    TestType::WeakPointer(INTERACTION.into()),
                )
                .field("PowerOnEffectTag", TestType::ReferenceTo(EFFECT.into()))
                .field("collisionDetection", TestType::Class(COLLISION.into())),
        )
}

fn controller() -> TestValue {
    TestValue::Struct(
        TestInstance::new(CONTROLLER)
            .value("fuelWarningDisplayTime", TestValue::Single(0.0))
            .value("disableESPInteraction", TestValue::Null)
            .value("PowerOnEffectTag", TestValue::Reference(3))
            .value(
                "collisionDetection",
                TestValue::Struct(
                    TestInstance::new(COLLISION)
                        .value("collisionWarnSpeed", TestValue::Single(3.0)),
                ),
            ),
    )
}

fn ifcs_schema(data: DataForgeBuilder) -> DataForgeBuilder {
    data.structure(
        TestStruct::new(POINT)
            .field("x", TestType::Single)
            .field("y", TestType::Single),
    )
    .structure(
        TestStruct::new(CURVE)
            .field("useLUT", TestType::Bool)
            .array("points", TestType::Class(POINT.into())),
    )
    .structure(
        TestStruct::new(BURN)
            .field("capacitorMax", TestType::Single)
            .field("afterburnerRampMapping", TestType::Class(CURVE.into())),
    )
    .structure(
        TestStruct::new(IFCS)
            .parent(BASE)
            .field("scmSpeed", TestType::Single)
            .field("maxSpeed", TestType::Single)
            .field("afterburner", TestType::Class(BURN.into()))
            .field("afterburnerNew", TestType::Class(BURN.into())),
    )
}

fn ifcs() -> TestValue {
    TestValue::Struct(
        TestInstance::new(IFCS)
            .value("scmSpeed", TestValue::Single(-0.0))
            .value("maxSpeed", TestValue::Single(f32::from_bits(RAW_NAN)))
            .value("afterburner", burn(10.0))
            .value("afterburnerNew", burn(20.0)),
    )
}

fn burn(capacity: f32) -> TestValue {
    let points = TestValue::Array(vec![point(0.25, 1.0), point(0.75, 2.0)]);
    let curve = TestInstance::new(CURVE)
        .value("useLUT", TestValue::Bool(false))
        .value("points", points);
    TestValue::Struct(
        TestInstance::new(BURN)
            .value("capacitorMax", TestValue::Single(capacity))
            .value("afterburnerRampMapping", TestValue::Struct(curve)),
    )
}

fn point(x: f32, y: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(POINT)
            .value("x", TestValue::Single(x))
            .value("y", TestValue::Single(y)),
    )
}

fn thruster(role: &str, capacity: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(THRUSTER)
            .value("thrusterType", TestValue::text(role))
            .value("thrustCapacity", TestValue::Single(-0.0))
            .value("thrustCapacityNew", TestValue::Single(capacity)),
    )
}
