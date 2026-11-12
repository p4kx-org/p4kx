// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportHealthStatus, Kind};
use p4kx_extraction::{DataType, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

const GUID: u128 = 0xe118;
const TYPE: &str = "MissionLocationTemplate";

#[test]
fn genuine_location_callback_admission_preserves_the_existing_readable_row() -> TestResult {
    let data = database();
    let mut inputs = Vec::new();
    for kind in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let input = Fixture::open(&data, kind, Vec::new())?;
        original(&input)?;
        inputs.push(input);
    }
    for input in inputs {
        check(&input)?;
    }
    Ok(())
}

fn check(input: &Fixture) -> TestResult {
    let built = p4kx_export::build_catalog_from(
        &input.archive,
        &input.entry,
        &health_origin_fixture::options(),
    )?;
    assert_eq!(
        built.summary().export_health().status,
        ExportHealthStatus::Complete,
        "readable original stays complete"
    );
    let counts = built
        .summary()
        .export_health()
        .kind(Kind::MissionLocationTemplates)
        .ok_or("actual LocationTemplate kind")?;
    assert_eq!(counts.source_records.selected, 1, "actual source selected");
    assert_eq!(
        counts.source_records.projected, 1,
        "actual source projected"
    );
    let document: serde_json::Value = serde_json::from_slice(built.json())?;
    let rows = document
        .get("missionLocationTemplates")
        .and_then(serde_json::Value::as_array)
        .ok_or("actual current LocationTemplate rows")?;
    assert_eq!(rows.len(), 1, "one original location row");
    let row = rows.first().ok_or("original location row")?;
    assert_eq!(
        row.get("guid").and_then(serde_json::Value::as_str),
        Some(health_origin_fixture::guid(GUID).as_str()),
        "actual original GUID"
    );
    assert_eq!(
        row.get("disabled"),
        Some(&serde_json::json!(true)),
        "same original flag value"
    );
    Ok(())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(TYPE).field(
            "locationData",
            TestType::Class("MissionLocationData".to_owned()),
        ))
        .structure(TestStruct::new("MissionLocationData").field("disabled", TestType::Bool))
        .record(
            TestRecord::new(
                GUID,
                TYPE,
                "MissionLocationTemplate.Original",
                "libs/foundry/records/missiondata/pu_locations/original.xml",
            )
            .value(
                "locationData",
                TestValue::Struct(
                    TestInstance::new("MissionLocationData")
                        .value("disabled", TestValue::Bool(true)),
                ),
            ),
        )
        .build()
}

fn original(input: &Fixture) -> TestResult {
    let forge = input.entry.open()?;
    let record = forge.record(0).ok_or("actual location Record")?;
    assert_eq!(
        record.guid().to_string(),
        health_origin_fixture::guid(GUID),
        "original header identity"
    );
    assert_eq!(record.type_name()?, TYPE, "genuine selected family");
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual paired Record/tree");
    assert_eq!(tree.error_count(), 0, "no unreadable original field");
    let field = tree
        .fields()
        .first()
        .ok_or("actual original locationData")?;
    assert_eq!(field.name(), "locationData", "actual selected declaration");
    assert_eq!(
        field.storage().data_type(),
        DataType::Class,
        "actual inline class storage"
    );
    assert!(!field.storage().is_array(), "actual scalar Class");
    let Value::Struct(data) = field.value() else {
        return Err("actual location Class".into());
    };
    assert_eq!(
        data.field("disabled"),
        Some(&Value::Bool(true)),
        "genuine original stored value"
    );
    Ok(())
}
