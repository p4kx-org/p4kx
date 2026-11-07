// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::{
    MediaError, archive_extras_read, archive_layout_projection, archive_records_read,
    archive_sections_read, archive_tables_read,
};

pub(crate) fn bind(mut base: Value) -> Result<Value, MediaError> {
    let rules = [
        (archive_layout_projection::LAYOUT_NAMESPACE, layout()),
        (archive_sections_read::SECTION_NAMESPACE, section()),
        (archive_records_read::RECORD_NAMESPACE, record()),
        (archive_extras_read::EXTRA_NAMESPACE, extra()),
        (archive_tables_read::INSTALL_NAMESPACE, installation()),
    ];
    let constraints = base
        .as_object_mut()
        .and_then(|root| root.get_mut("allOf"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media record allOf is not an array".into()))?;
    constraints.extend(rules.into_iter().map(|(namespace, data)| json!({
        "if":{"properties":{"kind":{"const":"archive"},"subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}},"required":["kind","subresource"]},
        "then":{"properties":{"data":data},"required":["data"]}
    })));
    Ok(base)
}

fn layout() -> Value {
    json!({"allOf":[
        {"if":{"properties":{"layout":{"enum":["zip","zip64"]}},"required":["layout"]},"then":{"properties":{"zip":{"type":"object"},"v2":{"type":"null"}}}},
        {"if":{"properties":{"layout":{"const":"p4k-v2"}},"required":["layout"]},"then":{"properties":{"v2":{"type":"object"},"zip":{"type":"null"}}}},
        {"if":{"properties":{"layout":{"const":"recovered-unknown"}},"required":["layout"]},"then":{"properties":{"v2":{"type":"null"},"zip":{"type":"null"}}}}
    ]})
}

fn section() -> Value {
    json!({"allOf":[
        {"if":{"properties":{"presence":{"const":"declared"}},"required":["presence"]},
         "then":{"properties":{"region":{"type":"object"}}},"else":{"properties":{"region":{"type":"null"},"raw":{"type":"null"}}}},
        {"if":{"properties":{"region":{"type":"object","properties":{"available":{"type":"object"}},"required":["available"]}},"required":["region"]},
         "then":{"properties":{"raw":{"type":"object"}}},"else":{"properties":{"raw":{"type":"null"}}}}
    ]})
}

fn record() -> Value {
    json!({"allOf":[
        {"if":{"properties":{"format":{"const":"p4k_v2"}},"required":["format"]},"then":{"properties":{"name":{"type":"null"},"extras":{"type":"null"},"comment":{"type":"null"},"signature":{"type":"object"},"storedDigest":{"type":"object"}}}},
        {"if":{"properties":{"format":{"const":"zip_central_directory"}},"required":["format"]},"then":{"properties":{"signature":{"type":"null"},"storedDigest":{"type":"null"}}}},
        {"if":{"properties":{"admittedEntryIndex":{"type":"null"}},"required":["admittedEntryIndex"]},"then":{"properties":{"metadata":{"type":"null"}}},"else":{"properties":{"metadata":{"type":"object"}}}}
    ]})
}

fn extra() -> Value {
    json!({"allOf":[
        {"if":{"properties":{"state":{"const":"short-header"}},"required":["state"]},"then":{"properties":{"declaredTotalLength":{"type":"null"},"body":{"type":"null"}}},"else":{"properties":{"id":{"type":"integer"},"declaredTotalLength":{"type":"integer"}}}},
        {"if":{"properties":{"state":{"const":"length-before-header"}},"required":["state"]},"then":{"properties":{"declaredTotalLength":{"maximum":3},"body":{"type":"null"}}}},
        {"if":{"properties":{"state":{"enum":["complete","truncated-body"]}},"required":["state"]},"then":{"properties":{"declaredTotalLength":{"minimum":4},"body":{"type":"object"}}}}
    ]})
}

fn installation() -> Value {
    json!({"allOf":[
        {"if":{"properties":{"reservedState":{"const":"not-present"}},"required":["reservedState"]},"then":{"properties":{"reserved":{"type":"null"}}}},
        {"if":{"properties":{"reservedState":{"const":"zero"}},"required":["reservedState"]},"then":{"properties":{"reserved":{"const":0}}}},
        {"if":{"properties":{"reservedState":{"const":"nonzero"}},"required":["reservedState"]},"then":{"properties":{"reserved":{"type":"integer","minimum":1}}}}
    ]})
}
