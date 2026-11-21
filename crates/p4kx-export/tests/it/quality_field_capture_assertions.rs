// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as origin;

pub(super) fn example<'catalog>(
    catalog: &'catalog Value,
    code: &str,
    field: &str,
    source_type: &str,
) -> TestResult<&'catalog Value> {
    let health = origin::at(catalog, "/exportHealth")?;
    for diagnostic in origin::rows(health, "/diagnostics")? {
        if diagnostic.get("code").and_then(Value::as_str) != Some(code) {
            continue;
        }
        for example in origin::rows(diagnostic, "/examples")? {
            if example.pointer("/recordIndex/value") != Some(&json!(0)) {
                continue;
            }
            let field_cell = origin::cell(example, "field", "present")?;
            let type_cell = origin::cell(example, "sourceType", "present")?;
            if origin::text_value(health, field_cell)? != &json!(field)
                || origin::text_value(health, type_cell)? != &json!(source_type)
            {
                continue;
            }
            origin::present_text(catalog, example, "field", &json!(field), Some(false))?;
            origin::present_text(
                catalog,
                example,
                "sourceType",
                &json!(source_type),
                Some(false),
            )?;
            origin::cell(example, "targetGuid", "absent")?;
            return Ok(example);
        }
    }
    Err(format!("the actual original {code} diagnostic for {field}/{source_type}").into())
}
