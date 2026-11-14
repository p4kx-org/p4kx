// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::{commodities, gameplay_properties, source_values};

pub(super) fn manufacturer() -> Value {
    json!({"availability": "available", "rows": [{
        "guid": source_values::present(json!("cfc0122d-5275-415a-a656-3dcb3346feb5")),
        "recordName": source_values::present(json!("SCItemManufacturer.ACAS")),
        "fileName": source_values::present(json!("libs/foundry/records/scitemmanufacturer/acas.xml")),
        "metadataTag": source_values::source(SourceValue::Absent),
        "className": source_values::present(json!("ACAS")),
        "recordPath": source_values::present(json!("libs/foundry/records/scitemmanufacturer/acas")),
        "recordIndex": source_values::present(json!(0)),
        "sourceType": source_values::present(json!("SCItemManufacturer")),
        "code": source_values::present(json!("ACAS")),
        "localization": source_values::present(json!({
            "sourceType": source_values::present(json!("SCItemLocalization")),
            "name": source_values::present(json!("name:22:@manufacturer_NameACAS")),
            "description": source_values::source(SourceValue::Null)
        }))
    }]})
}

pub(super) fn public_names() -> Vec<Value> {
    let mut rows = commodities::public_names();
    rows.extend(gameplay_properties::public_names());
    rows.push(json!({
        "id": source_values::present(json!("name:22:@manufacturer_NameACAS")),
        "key": source_values::present(json!("@manufacturer_NameACAS")),
        "translations": source_values::present(json!({
            "en": {
                "sourceText": source_values::present(json!("Ace Astrogation")),
                "displayText": source_values::present(json!("Ace Astrogation"))
            }
        }))
    }));
    rows
}
