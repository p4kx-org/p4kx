// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values::{present, source};

pub(super) fn definitions() -> Value {
    json!({"availability":"available", "rows":[{
        "guid": present(json!("00000000-0000-0000-0000-000000007002")),
        "recordName": present(json!("ResourceTypeGroup.Metal")),
        "className": present(json!("Metal")),
        "recordPath": present(json!("synthetic/resource_group")),
        "recordIndex": present(json!(7)),
        "sourceType": present(json!("ResourceTypeGroup")),
        "name": source(SourceValue::Absent),
        "description": source(SourceValue::Absent),
        "defaultThumbnailPath": present(json!("")),
        "defaultThumbnailPathSVG": source(SourceValue::Null)
    }]})
}

pub(super) fn members() -> Value {
    json!({"availability":"available", "rows":[{
        "guid": present(json!("00000000-0000-0000-0000-000000007002")),
        "recordIndex": present(json!(7)),
        "groups": present(json!([])),
        "resources": present(json!([source(SourceValue::Null)]))
    }]})
}

pub(super) fn parents() -> Value {
    json!({"availability":"available", "rows":[{
        "guid": present(json!("00000000-0000-0000-0000-000000007002")),
        "recordIndex": present(json!(7)),
        "parent": present(Value::Null)
    }]})
}
