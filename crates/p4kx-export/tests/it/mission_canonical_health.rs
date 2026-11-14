// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use crate::health_origin_assertions as health;
use crate::health_origin_fixture::TestResult;

/// Matches semantic example text only through the same actual report's typed text references.
pub(crate) fn example(
    catalog: &Value,
    code: Option<&str>,
    source_type: &str,
    field: impl Fn(&str) -> bool,
) -> TestResult<bool> {
    let report = health::at(catalog, "/exportHealth")?;
    for diagnostic in health::rows(report, "/diagnostics")? {
        if code.is_some_and(|code| diagnostic.get("code").and_then(Value::as_str) != Some(code)) {
            continue;
        }
        for example in health::rows(diagnostic, "/examples")? {
            let source = example
                .get("sourceType")
                .ok_or("required nullable source-type cell")?;
            let path = example
                .get("field")
                .ok_or("required nullable original field cell")?;
            if source.get("state").and_then(Value::as_str) != Some("present")
                || path.get("state").and_then(Value::as_str) != Some("present")
            {
                continue;
            }
            let source = health::text_value(report, source)?
                .as_str()
                .ok_or("actual source-type text")?;
            let path = health::text_value(report, path)?
                .as_str()
                .ok_or("actual original field text")?;
            if source == source_type && field(path) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
