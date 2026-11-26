// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestStruct, TestType,
    TestValue, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::wearable_fields;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const COMPONENT: &str = "DataForgeComponentParams";
const CLOTHING: &str = "SCItemClothingParams";
const TEMPERATURE: &str = "SCItemClothingTemperatureResistanceParams";
const RADIATION: &str = "SCItemClothingRadiationResistanceParams";
const RADAR: &str = "SCItemRadarComponentParams";
const AIM_ASSIST: &str = "SRadarAimAssistData";

#[test]
fn worn_fields_retain_selected_protection_and_aim_ranges_with_original_markers() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let output = wearable_fields::collect(&session.parsed(None)?, scratch.path(), None)?;
    let clothing = serde_json::to_value(output.clothing)?;
    let radars = serde_json::to_value(output.radars)?;
    let suit = row(&clothing, 0)?;
    let radar = row(&radars, 0)?;
    let radar_alias = row(&radars, 1)?;
    assert_eq!(suit.pointer("/componentOrdinal/value"), Some(&json!(2)));
    assert_eq!(radar.pointer("/componentOrdinal/value"), Some(&json!(1)));
    assert_eq!(
        radar_alias.pointer("/ownerRecordIndex/value"),
        Some(&json!(1))
    );
    assert_eq!(radar.get("ownerGuid"), radar_alias.get("ownerGuid"));
    assert_eq!(
        radar_alias.pointer("/aimAssist/value/distanceMinAssignment/value/finite"),
        Some(&json!(18.0))
    );
    assert!(
        radar
            .pointer("/aimAssist/value/outsideRangeBufferDistance")
            .is_none()
    );
    for (row, pointer, expected) in [
        (
            suit,
            "/temperatureResistance/value/minResistance",
            json!(-30.0),
        ),
        (
            suit,
            "/temperatureResistance/value/maxResistance",
            json!(90.0),
        ),
        (
            suit,
            "/radiationResistance/value/maximumRadiationCapacity",
            json!(500.0),
        ),
        (
            suit,
            "/radiationResistance/value/radiationDissipationRate",
            Value::Null,
        ),
        (radar, "/aimAssist/value/distanceMinAssignment", json!(4.0)),
        (radar, "/aimAssist/value/distanceMaxAssignment", json!(40.0)),
    ] {
        let cell = row.pointer(pointer).ok_or("selected wearable field")?;
        assert_eq!(cell.get("containsEncryptedSource"), Some(&json!(true)));
        assert_eq!(cell.pointer("/value/finite"), Some(&expected));
    }
    assert_eq!(
        suit.pointer("/radiationResistance/value/radiationDissipationRate/value/nonFinite"),
        Some(&json!("positiveInfinity"))
    );
    Ok(())
}

fn row(value: &Value, index: usize) -> Result<&Value, &'static str> {
    value
        .get("rows")
        .and_then(Value::as_array)
        .and_then(|rows| rows.get(index))
        .ok_or("selected original component row")
}

fn database() -> DataForgeBuilder {
    entity_schema(DataForgeBuilder::new().version(8))
        .structure(
            TestStruct::new(CLOTHING)
                .parent(COMPONENT)
                .field("TemperatureResistance", TestType::Class(TEMPERATURE.into()))
                .field("RadiationResistance", TestType::Class(RADIATION.into())),
        )
        .structure(
            TestStruct::new(TEMPERATURE)
                .field("MinResistance", TestType::Single)
                .field("MaxResistance", TestType::Single),
        )
        .structure(
            TestStruct::new(RADIATION)
                .field("MaximumRadiationCapacity", TestType::Single)
                .field("RadiationDissipationRate", TestType::Single),
        )
        .structure(
            TestStruct::new(RADAR)
                .parent(COMPONENT)
                .field("aimAssist", TestType::Class(AIM_ASSIST.into())),
        )
        .structure(
            TestStruct::new(AIM_ASSIST)
                .field("distanceMinAssignment", TestType::Single)
                .field("distanceMaxAssignment", TestType::Single)
                .field("outsideRangeBufferDistance", TestType::Single),
        )
        .record(
            item("First")
                .component(radar(4.0, 40.0))
                .component(TestValue::Struct(clothing()))
                .record(),
        )
        .record(item("Alias").component(radar(18.0, 90.0)).record())
}

fn radar(minimum: f32, maximum: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(RADAR).value(
            "aimAssist",
            TestValue::Struct(
                TestInstance::new(AIM_ASSIST)
                    .value("distanceMinAssignment", TestValue::Single(minimum))
                    .value("distanceMaxAssignment", TestValue::Single(maximum))
                    .value("outsideRangeBufferDistance", TestValue::Single(20.0)),
            ),
        ),
    )
}

fn clothing() -> TestInstance {
    TestInstance::new(CLOTHING)
        .value(
            "TemperatureResistance",
            TestValue::Struct(
                TestInstance::new(TEMPERATURE)
                    .value("MinResistance", TestValue::Single(-30.0))
                    .value("MaxResistance", TestValue::Single(90.0)),
            ),
        )
        .value(
            "RadiationResistance",
            TestValue::Struct(
                TestInstance::new(RADIATION)
                    .value("MaximumRadiationCapacity", TestValue::Single(500.0))
                    .value("RadiationDissipationRate", TestValue::Single(f32::INFINITY)),
            ),
        )
}

fn item(name: &str) -> TestEntity {
    TestEntity::new(
        0x1177_9011,
        format!("Fixture{name}"),
        format!("libs/foundry/records/entities/items/fixture/{name}.xml"),
    )
    .attach_type("Armor")
}
