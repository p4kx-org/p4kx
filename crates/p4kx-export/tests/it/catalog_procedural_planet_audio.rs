// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const SOURCE_TYPE: &str = "ProceduralPlanetAudioData";
const EXISTING: &str = "firstRoomIsPlanetRoomRtpc";
const ADDED: &str = "firstRoomIsPlanetRoomNotIncludingShipsRtpc";
const OTHER: &str = "firstRoomIsPlanetRoomOrVehicleRoomRtpc";

#[test]
fn original_planet_rtpc_occurrences_and_additional_fields_survive() -> TestResult {
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let marker = input.marker();
        let fixture = Fixture::open(&database(true), input, Vec::new())?;
        let (_, catalog) = fixture.diagnostic()?;
        let row = row(&catalog)?;
        origin::present(row, "sourceType", &json!(SOURCE_TYPE), marker)?;
        for (field, expected) in [(EXISTING, "synthetic_existing"), (ADDED, "")] {
            let carrier = origin::cell(row, field, "present")?;
            assert_eq!(
                carrier["containsEncryptedSource"],
                json!(marker),
                "the control uses the original archive context"
            );
            let rtpc = origin::at(carrier, "/value")?;
            let leaf = origin::present(rtpc, "rtpc", &json!(expected), marker)?;
            assert_ne!(
                carrier["sourceUsesRefs"], leaf["sourceUsesRefs"],
                "the control and its String have different original positions"
            );
            assert_eq!(
                origin::at(rtpc, "/additionalFields/roots/0/name/value")?,
                "extraScale",
                "the nested original extension remains available"
            );
        }
        let first = origin::cell(row, ADDED, "present")?;
        let second = origin::cell(row, OTHER, "unsupported")?;
        assert_ne!(
            first["sourceUsesRefs"], second["sourceUsesRefs"],
            "the two fields keep independent origin references"
        );
        assert_eq!(
            second["containsEncryptedSource"],
            json!(marker),
            "unsupported storage is located in the original source"
        );
        verify_collections(row, marker)?;
        let names: Vec<_> = origin::rows(row, "/additionalFields/roots")?
            .iter()
            .filter_map(|field| field.pointer("/name/value").and_then(Value::as_str))
            .collect();
        assert_eq!(
            names,
            ["futureRoot", OTHER],
            "additional fields keep source order"
        );
        origin::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn older_planet_rtpc_declarations_keep_absence_distinct_from_unreadable_text() -> TestResult {
    let fixture = Fixture::open(&database(false), Input::Encrypted, Vec::new())?;
    let (_, catalog) = fixture.diagnostic()?;
    let row = row(&catalog)?;
    for field in [ADDED, OTHER] {
        let cell = origin::cell(row, field, "absent")?;
        assert_eq!(
            cell["containsEncryptedSource"], true,
            "the missing field survey belongs to encrypted DataForge"
        );
        assert_eq!(
            cell["sourceSetComplete"], true,
            "the original scope was fully surveyed"
        );
    }
    let original = origin::cell(row, EXISTING, "present")?;
    let rtpc = origin::cell(origin::at(original, "/value")?, "rtpc", "unreadable")?;
    assert_eq!(
        rtpc["containsEncryptedSource"], true,
        "the unreadable field belongs to encrypted DataForge"
    );
    assert_eq!(
        rtpc["sourceSetComplete"], false,
        "the unreadable payload remains incomplete"
    );
    origin::closure(&catalog)
}

fn verify_collections(row: &Value, marker: Option<bool>) -> TestResult {
    for field in ["entries", "disturbanceLists", "effectTagMovementRtpcs"] {
        let cell = origin::cell(row, field, "present")?;
        assert_eq!(
            cell["containsEncryptedSource"],
            json!(marker),
            "the collection owns its original source field"
        );
        assert_eq!(
            origin::at(cell, "/value/roots/0/name/value")?,
            field,
            "the original subtree starts at its named field"
        );
    }
    Ok(())
}

fn row(catalog: &Value) -> TestResult<&Value> {
    let [row] = origin::rows(catalog, "/definitions/proceduralPlanetAudio/rows")? else {
        return Err("expected one retained physical planet audio record".into());
    };
    Ok(row)
}

fn database(extended: bool) -> Vec<u8> {
    let class = || TestType::Class("AudioRtpc".to_owned());
    let mut definition = TestStruct::new(SOURCE_TYPE)
        .field(EXISTING, class())
        .array("entries", TestType::String)
        .array("disturbanceLists", TestType::Reference)
        .array("effectTagMovementRtpcs", TestType::UInt32)
        .field("futureRoot", TestType::Double);
    let existing = if extended {
        TestValue::text("synthetic_existing")
    } else {
        TestValue::Raw(u32::MAX.to_le_bytes().to_vec())
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
    if extended {
        definition = definition
            .field(ADDED, class())
            .field(OTHER, TestType::Class("SyntheticOtherClass".to_owned()));
        record = record.value(ADDED, rtpc(TestValue::text(""), 2.5)).value(
            OTHER,
            TestValue::Struct(TestInstance::new("SyntheticOtherClass")),
        );
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(definition)
        .structure(
            TestStruct::new("AudioRtpc")
                .field("rtpc", TestType::String)
                .field("extraScale", TestType::Single),
        )
        .structure(TestStruct::new("SyntheticOtherClass"))
        .record(record)
        .build()
}

fn rtpc(name: TestValue, extra_scale: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("AudioRtpc")
            .value("rtpc", name)
            .value("extraScale", TestValue::Single(extra_scale)),
    )
}
