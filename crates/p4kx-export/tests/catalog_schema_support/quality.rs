// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Map, Value, json};

use super::source_values::{present, source};

fn header(guid: &str, index: u32, kind: &str, name: &str, path: &str) -> Map<String, Value> {
    Map::from_iter([
        ("guid".to_owned(), present(json!(guid))),
        (
            "recordName".to_owned(),
            present(json!(format!("{kind}.{name}"))),
        ),
        ("className".to_owned(), present(json!(name))),
        ("recordPath".to_owned(), present(json!(path))),
        ("recordIndex".to_owned(), present(json!(index))),
        ("sourceType".to_owned(), present(json!(kind))),
        ("fileName".to_owned(), present(json!(format!("{path}.xml")))),
        ("metadataTag".to_owned(), source(SourceValue::Absent)),
    ])
}

pub(super) fn distributions() -> Value {
    let mut row = header(
        "e2000000-0000-4000-8000-000000000001",
        30,
        "CraftingQualityDistributionRecord",
        "FixtureQuality",
        "crafting/qualitydistribution/fixture",
    );
    row.extend([
        ("qualityDistribution".to_owned(), present(curve())),
        ("locationOverride".to_owned(), source(SourceValue::Absent)),
    ]);
    json!({"availability":"available", "rows":[row]})
}

pub(super) fn curve() -> Value {
    json!({"sourceType":present(json!("CraftingQualityDistributionNormal")),
    "payload":present(json!({"type":"inline", "value":present(json!({
        "min":present(json!({"type":"int32", "value":1})),
        "max":present(json!({"type":"int32", "value":1000})),
        "mean":present(json!({"type":"f32", "rawBits":"3e000000", "finite":0.125})),
        "stddev":source(SourceValue::Null)
    }))}))})
}

pub(super) fn quantizations() -> Value {
    let mut row = header(
        "e4000000-0000-4000-8000-000000000001",
        31,
        "CraftingQualityQuantizationRecord",
        "FixtureQuantization",
        "crafting/qualityquantization/fixture",
    );
    row.insert(
        "qualityQuantization".to_owned(),
        present(json!({
            "sourceType":present(json!("CraftingQualityQuantization")),
            "payload":present(json!({"type":"inline", "value":present(json!({
                "bands":present(json!([present(json!({
                    "sourceType":present(json!("CraftingQualityQuantizationBand")),
                    "start":present(json!({"type":"int32", "value":0})),
                    "end":present(json!({"type":"int32", "value":1000})),
                    "mappedValue":present(json!({"type":"int32", "value":500}))
                }))]))
            }))}))
        })),
    );
    json!({"availability":"available", "rows":[row]})
}

pub(super) fn inverse() -> Value {
    json!({"availability":"available", "rows":[{
        "guid":present(json!("e4000000-0000-4000-8000-000000000001")),
        "recordIndex":present(json!(31)),
        "eligibility":present(json!({"status":"eligible"})),
        "resourceTypeGuids":present(json!([present(json!("b1000000-0000-4000-8000-000000000001"))]))
    }]})
}
