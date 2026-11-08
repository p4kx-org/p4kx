// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUses, SourceValue};
use serde_json::{Value, json};

pub(super) fn definitions() -> Value {
    json!({
        "resourceTypes": {
            "availability": "available",
            "rows": [{
                "guid": present(json!("00000000-0000-0000-0000-000000007001")),
                "className": present(json!("FixtureResource")),
                "recordPath": present(json!("libs/foundry/records/resourcetypedatabase/fixture_resource")),
                "name": source(SourceValue::Absent),
                "description": source(SourceValue::Absent),
                "groupGuids": present(json!([])),
                "refinedVersion": source(SourceValue::Null),
                "density": source(SourceValue::Null),
                "defaultThumbnailPath": source(SourceValue::Null),
                "defaultThumbnailPathSVG": source(SourceValue::Null),
                "rttThumbnailEntityClass": source(SourceValue::Null),
                "validateDefaultCargoBox": present(json!(false)),
                "defaultCargoContainers": source(SourceValue::Null),
                "properties": present(json!([
                    source(SourceValue::Null),
                    present(json!({
                        "sourceType": present(json!("ResourceTypeVolatility")),
                        "name": present(json!("")),
                        "payload": present(json!({
                            "type": "volatility",
                            "volatility": present(json!({"rawBits": "00000000", "finite": 0.0})),
                            "healthDecayPerSecond": present(json!({"rawBits": "00000000", "finite": 0.0}))
                        }))
                    }))
                ]))
            }]
        }
    })
}

fn present(value: Value) -> Value {
    source(SourceValue::Present(value))
}

fn source(value: SourceValue<Value>) -> Value {
    json!(SourceUses::new().value(value, &[], false))
}
