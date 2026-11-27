// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

pub(super) fn sets() -> Value {
    let mut properties = vec![
        Value::Null,
        json!({
            "variable": "", "token": null, "valueType": null, "value": null
        }),
    ];
    if let Value::Array(variants) = super::contract_properties::template_properties() {
        properties.extend(variants);
    }
    json!([{
        "guid": "a7000000-0000-4000-8000-000000000001",
        "recordGuid": "10000000-0000-4000-8000-000000000001",
        "recordPath": "libs/foundry/records/contracts/contractgenerator/delivery",
        "sourceType": "ContractGenerator",
        "sourceLocation": "handlers[0].contracts[0].paramOverrides.propertyOverrides",
        "level": "contract",
        "properties": properties
    }])
}
