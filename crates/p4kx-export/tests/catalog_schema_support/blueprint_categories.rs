// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

pub(super) fn definitions() -> Value {
    json!({"availability":"available", "rows":[{
        "guid":present(json!("d1000000-0000-4000-8000-000000000001")),
        "recordName":present(json!("BlueprintCategoryRecord.Ammo")),
        "className":present(json!("Ammo")),
        "recordPath":present(json!("libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase")),
        "recordIndex":present(json!(0)),
        "sourceType":present(json!("BlueprintCategoryRecord")),
        "fileName":present(json!("libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase.xml")),
        "metadataTag":source(SourceValue::Absent)
    }]})
}
