// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

pub(super) fn crafters() -> Value {
    let guid = "7000f0f0-0000-4000-8000-000000000001";
    let path = "libs/foundry/records/entities/basebuilding/item_fabricator";
    json!({"availability": "available", "rows": [{
        "guid": present(json!(guid)),
        "recordName": present(json!("EntityClassDefinition.item_fabricator")),
        "className": present(json!("item_fabricator")),
        "recordPath": present(json!(path)),
        "recordIndex": present(json!(0)),
        "sourceType": present(json!("EntityClassDefinition")),
        "fileName": present(json!(format!("{path}.xml"))),
        "metadataTag": source(SourceValue::Absent),
        "component": present(json!({
            "componentOrdinal": present(json!(0)),
            "sourceType": present(json!("CrafterComponentParams")),
            "additionalFields": {"roots": [], "values": []},
            "maxJobsInHistory": present(json!({"type":"int32", "value":10})),
            "baseCraftingTime": source(SourceValue::Absent),
            "queues": present(json!([])),
            "modifiers": source(SourceValue::Absent),
        })),
        "attachment": source(SourceValue::Absent)
    }]})
}
