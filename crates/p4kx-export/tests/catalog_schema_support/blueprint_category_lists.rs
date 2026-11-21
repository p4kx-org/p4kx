// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

pub(super) fn declarations() -> Value {
    json!({"availability":"available", "rows":[{
        "guid":present(json!("bd100000-0000-4000-8000-000000000001")),
        "recordName":present(json!("BlueprintCategoryDatabaseRecord.AmmoList")),
        "className":present(json!("AmmoList")),
        "recordPath":present(json!("synthetic/category_lists/ammo")),
        "recordIndex":present(json!(1)),
        "sourceType":present(json!("BlueprintCategoryDatabaseRecord")),
        "fileName":present(json!("synthetic/category_lists/ammo.xml")),
        "metadataTag":source(SourceValue::Absent),
        "categories":present(json!([
            present(json!({
                "guid":present(json!("d1000000-0000-4000-8000-000000000001")),
                "targetRecordIndex":present(json!(0)),
                "targetType":present(json!("BlueprintCategoryRecord")),
                "recordPath":present(json!("libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase")),
                "status":"present"
            })),
            source(SourceValue::Null)
        ]))
    }]})
}
