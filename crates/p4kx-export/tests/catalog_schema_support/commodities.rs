// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

const GUID: &str = "a1000000-0000-4000-8000-000000000001";
const NAME_ID: &str = "name:23:@items_commodities_gold";

pub(super) fn definitions() -> Value {
    json!({"availability": "available", "rows": [{
        "guid": present(json!(GUID)),
        "recordName": present(json!("EntityClassDefinition.gold")),
        "fileName": present(json!("libs/foundry/records/entities/commodities/metals/gold.xml")),
        "metadataTag": source(SourceValue::Absent),
        "className": present(json!("gold")),
        "recordPath": present(json!("libs/foundry/records/entities/commodities/metals/gold")),
        "recordIndex": present(json!(0)),
        "sourceType": present(json!("EntityClassDefinition")),
        "selection": present(json!("entity")),
        "selectionDiagnostics": [],
        "entity": present(json!({
            "purchasableParameters": present(json!([])),
            "displayParameters": present(json!([])),
            "resourceContainers": present(json!([]))
        }))
    }]})
}

pub(super) fn presentation() -> Value {
    json!({"availability": "available", "rows": [{
        "commodityGuid": present(json!(GUID)),
        "name": present(json!(NAME_ID)),
        "description": source(SourceValue::Null),
        "displayType": source(SourceValue::Null),
        "typeGuid": present(json!("b1000000-0000-4000-8000-000000000001")),
        "subtypeGuid": source(SourceValue::Null),
        "densityGramsPerCubicCentimetre": source(SourceValue::Null),
        "refinedSubtypeGuid": source(SourceValue::Null)
    }]})
}

pub(super) fn public_names() -> Vec<Value> {
    vec![json!({
        "id": present(json!(NAME_ID)),
        "key": present(json!("@items_commodities_gold")),
        "translations": present(json!({"en": {
            "sourceText": present(json!("Gold")),
            "displayText": present(json!("Gold"))
        }}))
    })]
}
