// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use super::TestResult;

pub(super) const GLOBAL: u32 = 0;
pub(super) const SUBTYPE: u32 = 1;
pub(super) const ITEM: u32 = 2;
pub(super) const ALIAS: u32 = 3;
const GLOBAL_TYPE: &str = "MiningLaserGlobalParams";
const SUBTYPE_TYPE: &str = "ConsumableSubtype";
const CONSUMABLE: &str = "SCItemConsumableParams";
const LASER: &str = "SEntityComponentMiningLaserParams";
const ATTACHABLE: &str = "EntityComponentAttachableModifierParams";
const MODIFIER: &str = "ItemModifiersParams";
const WEAPON: &str = "ItemWeaponModifiersParams";
const OPAQUE: &str = "FixtureOtherModifier";
const CONTENT: &str = "FixtureContents";
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "FixtureComponent";
const BASE_UNIT: &str = "SBaseCargoUnit";
const MICRO: &str = "SMicroCargoUnit";
const OWNER_GUID: u128 = 1;
const GLOBAL_GUID: u128 = 2;
const SUBTYPE_GUID: u128 = 3;
const DANGLING_GUID: u128 = 4;

pub(super) fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let database = schema()
        .record(
            TestRecord::new(
                GLOBAL_GUID,
                GLOBAL_TYPE,
                "FixtureGlobal",
                "fixtures/tools/global.xml",
            )
            .value("throttleChangePerAction", TestValue::Single(0.25)),
        )
        .record(
            TestRecord::new(
                SUBTYPE_GUID,
                SUBTYPE_TYPE,
                "FixtureSubtype",
                "fixtures/tools/subtype.xml",
            )
            .value("typeName", TestValue::text("FixtureSubtype")),
        )
        .record(entity(
            "item",
            vec![
                TestValue::Null,
                laser(0.5, GLOBAL_GUID),
                consumable(),
                attachable(),
            ],
        ))
        .record(entity("alias", vec![laser(-0.0, DANGLING_GUID)]))
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
        .structure(TestStruct::new(COMPONENT))
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(GLOBAL_TYPE).field("throttleChangePerAction", TestType::Single))
        .structure(TestStruct::new(SUBTYPE_TYPE).field("typeName", TestType::String))
        .structure(
            TestStruct::new(LASER)
                .parent(COMPONENT)
                .field("throttleMinimum", TestType::Single)
                .field("globalParams", reference(GLOBAL_TYPE)),
        )
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("charges", TestType::Int32)
                .array("modifiers", pointer(MODIFIER)),
        )
        .structure(TestStruct::new(MODIFIER))
        .structure(
            TestStruct::new(WEAPON)
                .parent(MODIFIER)
                .field("fireActionIndex", TestType::Int32),
        )
        .structure(
            TestStruct::new(OPAQUE)
                .parent(MODIFIER)
                .field("fixtureExtra", TestType::Int32),
        )
        .structure(TestStruct::new(BASE_UNIT))
        .structure(
            TestStruct::new(MICRO)
                .parent(BASE_UNIT)
                .field("microSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new(CONTENT)
                .field("consumableSubtype", reference(SUBTYPE_TYPE))
                .field("ratio", TestType::Single),
        )
        .structure(
            TestStruct::new(CONSUMABLE)
                .parent(COMPONENT)
                .array("defaultContents", TestType::Class(CONTENT.into()))
                .field("consumableVolume", pointer(BASE_UNIT)),
        )
}

fn entity(name: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        OWNER_GUID,
        ENTITY,
        name,
        format!("fixtures/tools/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn laser(minimum: f32, global: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new(LASER)
            .value("throttleMinimum", TestValue::Single(minimum))
            .value("globalParams", TestValue::Reference(global)),
    )
}

fn consumable() -> TestValue {
    TestValue::Struct(
        TestInstance::new(CONSUMABLE)
            .value(
                "defaultContents",
                TestValue::Array(vec![
                    content(SUBTYPE_GUID, 1.0),
                    content(DANGLING_GUID, -0.0),
                ]),
            )
            .value(
                "consumableVolume",
                TestValue::Struct(TestInstance::new(MICRO).value("microSCU", TestValue::Int32(19))),
            ),
    )
}

fn content(subtype: u128, ratio: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(CONTENT)
            .value("consumableSubtype", TestValue::Reference(subtype))
            .value("ratio", TestValue::Single(ratio)),
    )
}

fn attachable() -> TestValue {
    TestValue::Struct(
        TestInstance::new(ATTACHABLE)
            .value("charges", TestValue::Int32(3))
            .value(
                "modifiers",
                TestValue::Array(vec![
                    TestValue::Struct(
                        TestInstance::new(WEAPON).value("fireActionIndex", TestValue::Int32(2)),
                    ),
                    TestValue::Null,
                    TestValue::Struct(
                        TestInstance::new(OPAQUE).value("fixtureExtra", TestValue::Int32(-9)),
                    ),
                ]),
            ),
    )
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.into())
}

fn reference(name: &str) -> TestType {
    TestType::ReferenceTo(name.into())
}
