// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;

pub(super) const FIRST: u32 = 1;
pub(super) const ALIAS: u32 = 2;
pub(super) const RADAR_TARGET: u32 = 3;
pub(super) const RADAR_TARGET_ALIAS: u32 = 4;
const ENTITY: &str = "EntityClassDefinition";
const BASE: &str = "FixtureComponent";
const SUIT: &str = "SCItemSuitArmorParams";
const MACRO: &str = "DamageResistanceMacro";
const CLOTHING: &str = "SCItemClothingParams";
const TEMPERATURE: &str = "SCItemClothingTemperatureResistanceParams";
const RADAR: &str = "SCItemRadarComponentParams";
const PING: &str = "SCItemRadarPingProperties";
const SHARED_RADAR: &str = "RadarSystemSharedParams";
const RADAR_SETTINGS: &str = "FixtureRadarSettings";
const OWNER_GUID: u128 = 1;
const MACRO_GUID: u128 = 2;
const DANGLING_GUID: u128 = 3;
const RADAR_TARGET_GUID: u128 = 4;

pub(super) fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let database = schema()
        .record(
            TestRecord::new(MACRO_GUID, MACRO, "FixtureMacro", "fixtures/worn/macro.xml")
                .value("FixtureFlag", TestValue::Bool(true)),
        )
        .record(entity(
            "first",
            vec![suit(2.0, MACRO_GUID), TestValue::Null, clothing(), radar()],
        ))
        .record(entity(
            "alias",
            vec![TestValue::Null, suit(-0.0, DANGLING_GUID)],
        ))
        .record(shared_radar("shared", true))
        .record(shared_radar("shared_alias", false))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(r"Data\Game2.dcb", database).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((directory, archive))
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(BASE.into())),
        )
        .structure(TestStruct::new(MACRO).field("FixtureFlag", TestType::Bool))
        .structure(
            TestStruct::new(SUIT)
                .parent(BASE)
                .field("integrityMilestoneToBreak", TestType::Single)
                .field("damageResistance", TestType::ReferenceTo(MACRO.into())),
        )
        .structure(
            TestStruct::new(CLOTHING)
                .parent(BASE)
                .field("TemperatureResistance", TestType::Class(TEMPERATURE.into())),
        )
        .structure(TestStruct::new(TEMPERATURE).field("MinResistance", TestType::Single))
        .structure(
            TestStruct::new(RADAR)
                .parent(BASE)
                .field("pingProperties", TestType::Class(PING.into()))
                .field("sharedParams", TestType::ReferenceTo(SHARED_RADAR.into()))
                .field(
                    "sharedExperimentalParams",
                    TestType::ReferenceTo(SHARED_RADAR.into()),
                ),
        )
        .structure(TestStruct::new(PING).field("cooldownTime", TestType::Single))
        .structure(
            TestStruct::new(SHARED_RADAR).field("settings", TestType::Class(RADAR_SETTINGS.into())),
        )
        .structure(TestStruct::new(RADAR_SETTINGS).field("flag", TestType::Bool))
}

fn entity(name: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        OWNER_GUID,
        ENTITY,
        name,
        format!("fixtures/worn/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn suit(integrity: f32, definition: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new(SUIT)
            .value("integrityMilestoneToBreak", TestValue::Single(integrity))
            .value("damageResistance", TestValue::Reference(definition)),
    )
}

fn clothing() -> TestValue {
    TestValue::Struct(TestInstance::new(CLOTHING).value(
        "TemperatureResistance",
        TestValue::Struct(
            TestInstance::new(TEMPERATURE).value("MinResistance", TestValue::Single(-5.0)),
        ),
    ))
}

fn radar() -> TestValue {
    TestValue::Struct(
        TestInstance::new(RADAR)
            .value(
                "pingProperties",
                TestValue::Struct(
                    TestInstance::new(PING).value("cooldownTime", TestValue::Single(8.0)),
                ),
            )
            .value("sharedParams", TestValue::Reference(RADAR_TARGET_GUID))
            .value(
                "sharedExperimentalParams",
                TestValue::Reference(DANGLING_GUID),
            ),
    )
}

fn shared_radar(name: &str, flag: bool) -> TestRecord {
    TestRecord::new(
        RADAR_TARGET_GUID,
        SHARED_RADAR,
        name,
        format!("fixtures/worn/{name}.xml"),
    )
    .value(
        "settings",
        TestValue::Struct(TestInstance::new(RADAR_SETTINGS).value("flag", TestValue::Bool(flag))),
    )
}
