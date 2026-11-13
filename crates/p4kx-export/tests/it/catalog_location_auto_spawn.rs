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

fn database() -> DataForgeBuilder {
    let owner = "MissionLocationTemplate";
    let location = "MissionLocationData";
    DataForgeBuilder::new()
        .version(8)
        .enumeration("EExcludeSpawnGender", ["FixtureNone"])
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new("TagList").array("tags", TestType::ReferenceTo("Tag".into())))
        .structure(TestStruct::new("SpawnSettingsInventoryItem"))
        .structure(settings())
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
                TestValue::Struct(
                    TestInstance::new(location)
                        .value("autoSpawnSettings", TestValue::Array(vec![spawn()])),
                ),
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

fn spawn() -> TestValue {
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
    TestValue::Struct(
        TestInstance::new(SETTINGS)
            .value("excludeSpawnGender", TestValue::text("FixtureNone"))
            .value("minGroupSize", TestValue::Int32(-1))
            .value("minSpawnDelay", TestValue::Single(-0.0))
            .value(
                "maxSpawnDelay",
                TestValue::Single(f32::from_bits(0x7fc0_4321)),
            )
            .value("inventoryItems", TestValue::Array(Vec::new()))
            .value("positiveCharacterTags", positive)
            .value("negativeCharacterTags", negative),
    )
}
