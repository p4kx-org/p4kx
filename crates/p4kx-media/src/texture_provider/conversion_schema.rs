// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::conversion_payload::{CONVERSION, IMAGE};
pub(super) fn bind(branches: &mut Vec<Value>, payload: &str) {
    for (namespace, record_type, indices) in [(IMAGE, "image", 2), (CONVERSION, "conversion", 1)] {
        let status = if namespace == IMAGE {
            json!({"const":"complete"})
        } else {
            json!({"enum":["complete","partial"]})
        };
        let checks = if namespace == CONVERSION {
            vec![
                conversion_status("written", "complete"),
                conversion_status("failed", "partial"),
            ]
        } else {
            vec![json!({"properties":{"status":{"const":"complete"}}})]
        };
        branches.push(json!({
            "if":{"properties":{
                "kind":{"const":"texture"},
                "subresource":{"properties":{"namespace":{"const":namespace}},"required":["namespace"]}
            },"required":["kind","subresource"]},
            "then":{"properties":{
                "data":{"allOf":[
                    {"$ref":format!("#/$defs/{payload}")},
                    {"properties":{"record_type":{"const":record_type}},"required":["record_type"]}
                ]},
                "subresource":{"properties":{"indices":{"minItems":indices,"maxItems":indices}}},
                "status":status
            },"required":["data"], "allOf": checks}
        }));
    }
}

fn conversion_status(outcome: &str, status: &str) -> Value {
    let outcome = json!({"properties":{"status":{"const":outcome}}});
    let payload = json!({"properties":{"outcome":outcome}});
    let data = json!({"properties":{"payload":payload}});
    json!({
        "if":{"properties":{"data":data}},
        "then":{"properties":{"status":{"const":status}}}
    })
}
