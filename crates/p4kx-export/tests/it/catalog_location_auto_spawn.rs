// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

const OWNER: u128 = 0x1074_0101;
const TAG: u128 = 0x1074_01ff;
const SETTINGS: &str = "AutoSpawnSettings";

#[test]
fn location_spawn_settings_preserve_source_bits_and_tag_order() -> TestResult {
    let (_directory, archive) = common::open(&database(), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &common::options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let spawn = at(
        &catalog,
        "/missionLocationTemplates/0/configuration/value/autoSpawnSettings/value/0/value",
    )?;
    for (path, value) in [
        ("/excludeSpawnGender/value", json!("FixtureNone")),
        ("/minGroupSize/value", json!(-1)),
        ("/minSpawnDelay/value/rawBits", json!("80000000")),
        ("/maxSpawnDelay/value/rawBits", json!("7fc04321")),
        ("/maxSpawnDelay/value/finite", Value::Null),
        ("/maxSpawnDelay/containsEncryptedSource", json!(true)),
        ("/inventoryItems/value", json!([])),
        (
            "/positiveCharacterTags/value/tags/value/0/value/guid/value",
            json!(guid(TAG)),
        ),
        (
            "/positiveCharacterTags/value/tags/value/1/state",
            json!("null"),
        ),
        (
            "/positiveCharacterTags/value/tags/value/2/value/guid/value",
            json!(guid(TAG)),
        ),
        ("/negativeCharacterTags/value/tags/value", json!([])),
    ] {
        assert_eq!(at(spawn, path)?, &value, "original spawn value at {path}");
    }
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        "complete",
        "the exact nonfinite source bits are covered in normal publication"
    );
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

pub(super) fn database() -> DataForgeBuilder {
    database_with_spawn(
        TestStruct::new("SpawnSettingsInventoryItem"),
        settings(),
        spawn(Vec::new()),
    )
}

fn database_with_spawn(
    inventory: TestStruct,
    settings: TestStruct,
    spawn: TestInstance,
) -> DataForgeBuilder {
    let owner = "MissionLocationTemplate";
    let location = "MissionLocationData";
    DataForgeBuilder::new()
        .version(8)
        .enumeration("EExcludeSpawnGender", ["FixtureNone"])
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new("TagList").array("tags", TestType::ReferenceTo("Tag".into())))
        .structure(inventory)
        .structure(settings)
        .structure(
            TestStruct::new(location).array("autoSpawnSettings", TestType::Class(SETTINGS.into())),
        )
        .structure(TestStruct::new(owner).field("locationData", TestType::Class(location.into())))
        .record(
            TestRecord::new(
                OWNER,
                owner,
                "MissionLocationTemplate.FixtureSpawn",
                "libs/foundry/records/missiondata/pu_locations/fixture/spawns.xml",
            )
            .value(
                "locationData",
                TestValue::Struct(TestInstance::new(location).value(
                    "autoSpawnSettings",
                    TestValue::Array(vec![TestValue::Struct(spawn)]),
                )),
            ),
        )
}

fn settings() -> TestStruct {
    let gender = "EExcludeSpawnGender";
    TestStruct::new(SETTINGS)
        .field("excludeSpawnGender", TestType::Enum(gender.into()))
        .field("minGroupSize", TestType::Int32)
        .field("minSpawnDelay", TestType::Single)
        .field("maxSpawnDelay", TestType::Single)
        .array(
            "inventoryItems",
            TestType::Class("SpawnSettingsInventoryItem".into()),
        )
        .field("positiveCharacterTags", TestType::Class("TagList".into()))
        .field("negativeCharacterTags", TestType::Class("TagList".into()))
}

fn spawn(inventory_items: Vec<TestValue>) -> TestInstance {
    let positive = TestValue::Struct(TestInstance::new("TagList").value(
        "tags",
        TestValue::Array(vec![
            TestValue::Reference(TAG),
            TestValue::Null,
            TestValue::Reference(TAG),
        ]),
    ));
    let negative =
        TestValue::Struct(TestInstance::new("TagList").value("tags", TestValue::Array(Vec::new())));
    TestInstance::new(SETTINGS)
        .value("excludeSpawnGender", TestValue::text("FixtureNone"))
        .value("minGroupSize", TestValue::Int32(-1))
        .value("minSpawnDelay", TestValue::Single(-0.0))
        .value(
            "maxSpawnDelay",
            TestValue::Single(f32::from_bits(0x7fc0_4321)),
        )
        .value("inventoryItems", TestValue::Array(inventory_items))
        .value("positiveCharacterTags", positive)
        .value("negativeCharacterTags", negative)
}

pub(super) fn inventory_database() -> DataForgeBuilder {
    let mut settings = complete_settings();
    let mut spawn = complete_spawn();
    for (name, reference) in additional_tag_fields().into_iter().zip(0x1166_0400_u128..) {
        settings = settings.field(name, TestType::Class("TagList".into()));
        spawn = spawn.value(
            name,
            TestValue::Struct(TestInstance::new("TagList").value(
                "tags",
                TestValue::Array(vec![TestValue::Reference(reference)]),
            )),
        );
    }
    database_with_spawn(inventory_schema(), settings, spawn)
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new("Faction"))
}

fn inventory_schema() -> TestStruct {
    TestStruct::new("SpawnSettingsInventoryItem")
        .field(
            "entityClass",
            TestType::ReferenceTo("EntityClassDefinition".into()),
        )
        .field("quantity", TestType::Int32)
        .field("amountInventoriesToFill", TestType::Int32)
        .field("markAsMissionItem", TestType::Bool)
        .field("fixtureAdditional", TestType::Bool)
}

fn inventory_item() -> TestValue {
    TestValue::Struct(
        TestInstance::new("SpawnSettingsInventoryItem")
            .value("entityClass", TestValue::Null)
            .value("quantity", TestValue::Int32(-1))
            .value("amountInventoriesToFill", TestValue::Int32(0))
            .value("markAsMissionItem", TestValue::Bool(false))
            .value("fixtureAdditional", TestValue::Bool(true)),
    )
}

fn complete_settings() -> TestStruct {
    settings()
        .field("name", TestType::String)
        .field("initialActivity", TestType::String)
        .field("excludeShipCrew", TestType::Bool)
        .field("maxGroupSize", TestType::Int32)
        .field("maxConcurrentSpawns", TestType::Int32)
        .field("maxSpawns", TestType::Int32)
        .field("factionOverride", TestType::ReferenceTo("Faction".into()))
        .field("missionAlliedMarker", TestType::Bool)
        .field("isCritical", TestType::Bool)
        .field("fixtureSpawnExtra", TestType::Bool)
}

fn complete_spawn() -> TestInstance {
    spawn(vec![inventory_item(), inventory_item()])
        .value("name", TestValue::text("FixtureSpawnName"))
        .value("initialActivity", TestValue::text("FixtureSpawnActivity"))
        .value("excludeShipCrew", TestValue::Bool(true))
        .value("maxGroupSize", TestValue::Int32(5))
        .value("maxConcurrentSpawns", TestValue::Int32(7))
        .value("maxSpawns", TestValue::Int32(11))
        .value("factionOverride", TestValue::Reference(0x1166_0301))
        .value("missionAlliedMarker", TestValue::Bool(false))
        .value("isCritical", TestValue::Bool(true))
        .value("fixtureSpawnExtra", TestValue::Bool(false))
}

const fn additional_tag_fields() -> [&'static str; 9] {
    [
        "closetPositiveTags",
        "closetNegativeTags",
        "roomPositiveTags",
        "roomNegativeTags",
        "defendAreaPositiveTags",
        "defendAreaNegativeTags",
        "scheduleAreaPositiveTags",
        "scheduleAreaNegativeTags",
        "entityTags",
    ]
}
