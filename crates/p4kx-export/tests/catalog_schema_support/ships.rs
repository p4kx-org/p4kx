// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

pub(super) fn rows() -> Value {
    let absent = || source(SourceValue::Absent);
    json!([{
        "guid": present(json!("90000000-0000-4000-8000-000000000001")),
        "recordIndex": present(json!(2)),
        "recordName": present(json!("EntityClassDefinition.ACAS_Ship")),
        "className": present(json!("ACAS_Ship")),
        "recordPath": present(json!("libs/foundry/records/entities/spaceships/acas_ship")),
        "fileName": present(json!("libs/foundry/records/entities/spaceships/acas_ship.xml")),
        "sourceType": present(json!("EntityClassDefinition")),
        "metadataTag": absent(),
        "name": absent(),
        "description": absent(),
        "role": present(json!("name:27:@vehicle_class_lightfreight")),
        "career": absent(),
        "crewSize": present(json!({"type": "int32", "value": 2})),
        "vehicleManufacturer": {"componentOrdinal": absent(), "manufacturer": absent()},
        "attachmentManufacturer": {
            "componentOrdinal": absent(), "attachDefPresent": absent(), "manufacturer": absent()
        },
        "vehicleDefinition": absent(),
        "vehicleKind": present(json!("spaceship")),
        "size": present(json!({"type": "int32", "value": 3})),
        "mass": present(json!(1210.0)),
        "massBasis": present(json!("parts")),
        "hullHealth": present(json!(600.0)),
        "cargoScu": 18.5,
        "cargoGridsWithIssue": 0,
        "playability": present(json!("ai")),
        "variantOf": "a0000000-0000-4000-8000-000000000001",
        "insurance": present(json!({
            "staticElementOrdinal": present(json!(0)),
            "restoreOnClaim": present(json!(true)),
            "shipInsuranceParams": present(json!({
                "shipEntityClassName": present(json!("AEGS_Avenger_Titan")),
                "baseWaitTimeMinutes": single(8.5),
                "mandatoryWaitTimeMinutes": single(2.5),
                "expediteFee": present(json!({"type": "uint32", "value": 3200})),
                "additionalFields": {"roots": [], "values": []}
            })),
            "additionalFields": {"roots": [], "values": []}
        })),
        "playabilityInputs": {
            "crewManifestComponentOrdinal": absent(), "crewManifest": absent()
        }
    }])
}

fn single(value: f32) -> Value {
    present(json!({
        "type": "f32", "rawBits": format!("{:08x}", value.to_bits()), "finite": f64::from(value)
    }))
}
