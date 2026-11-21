// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::conversion_payload::ConversionPayload;
use super::conversion_schema;
use super::family_payload::{EXPECTED, FAMILY, FamilyPayload, MEMBER};
use super::header::{NAMESPACE, TextureHeaderPayload};
use crate::{MediaError, schema_merge};

pub(crate) fn bind_record_schema(mut base: Value) -> Result<Value, MediaError> {
    const PAYLOAD: &str = "TextureHeaderPayload";
    const FAMILY_PAYLOAD: &str = "TextureFamilyPayload";
    const CONVERSION_PAYLOAD: &str = "TextureConversionPayload";
    schema_merge::insert_payload(
        &mut base,
        PAYLOAD,
        schema_merge::serialized_payload::<TextureHeaderPayload>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        FAMILY_PAYLOAD,
        schema_merge::serialized_payload::<FamilyPayload>(),
    )?;
    schema_merge::insert_payload(
        &mut base,
        CONVERSION_PAYLOAD,
        schema_merge::serialized_payload::<ConversionPayload>(),
    )?;
    let branches = base
        .get_mut("allOf")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| MediaError::Contract("media schema has no built-in constraints".into()))?;
    branches.push(json!({
        "if": {
            "properties": {
                "kind": {"const":"texture"},
                "subresource": {"properties":{"namespace":{"const":NAMESPACE}}, "required":["namespace"]}
            },
            "required":["kind","subresource"]
        },
        "then": {
            "properties": {
                "data":{"$ref":format!("#/$defs/{PAYLOAD}")},
                "subresource":{"properties":{"indices":{"maxItems":0}}}
            },
            "allOf": [
                {
                    "if":{"properties":{"data":{"properties":{"record_type":{"const":"header"}}}}},
                    "then":{"properties":{"status":{"const":"complete"}}}
                },
                {
                    "if":{"properties":{"data":{"properties":{"record_type":{"const":"invalid-header"}}}}},
                    "then":{"properties":{"status":{"const":"partial"}}}
                }
            ]
        }
    }));
    family_branches(branches, FAMILY_PAYLOAD)?;
    conversion_schema::bind(branches, CONVERSION_PAYLOAD);
    Ok(base)
}

fn family_branches(branches: &mut Vec<Value>, payload: &str) -> Result<(), MediaError> {
    for (namespace, record_type, indices) in [
        (MEMBER, "member", 1),
        (EXPECTED, "expected-part", 1),
        (FAMILY, "family", 0),
    ] {
        let mut branch = json!({
            "if": {
                "properties": {
                    "kind":{"const":"texture"},
                    "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
                },
                "required":["kind","subresource"]
            },
            "then": {"properties": {
                "data":{"allOf":[
                    {"$ref":format!("#/$defs/{payload}")},
                    {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
                ]},
                "subresource":{"properties":{"indices":{"minItems":indices,"maxItems":indices}}},
                "status": if namespace == FAMILY { json!({"enum":["complete","partial"]}) } else { json!({"const":"complete"}) }
            }, "required":["data"]}
        });
        if namespace == FAMILY {
            bind_family_status(&mut branch)?;
        }
        branches.push(branch);
    }
    Ok(())
}

fn bind_family_status(branch: &mut Value) -> Result<(), MediaError> {
    let constraints = branch
        .pointer_mut("/then")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            MediaError::Contract("texture family schema has no record constraints".into())
        })?;
    constraints.insert("allOf".into(), json!([
        {
            "if":{"properties":{"data":{"properties":{"payload":{"properties":{"directory":{"properties":{"status":{"const":"complete"}}}}}}}}},
            "then":{"properties":{"status":{"const":"complete"}}}
        },
        {
            "if":{"properties":{"data":{"properties":{"payload":{"properties":{"directory":{"properties":{"status":{"const":"incomplete"}}}}}}}}},
            "then":{"properties":{"status":{"const":"partial"}}}
        }
    ]));
    Ok(())
}
