// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::Value;

use crate::catalog_v3_consumer;

pub(super) fn decimal(carrier: &Value) -> Result<f64, Box<dyn Error>> {
    let number = catalog_v3_consumer::present(carrier)?;
    let decimal = match number["type"].as_str() {
        Some("f32" | "f64") => number["finite"].as_f64(),
        Some("int64" | "uint64") => number["value"]
            .as_str()
            .and_then(|value| value.parse().ok()),
        _ => number["value"].as_f64(),
    };
    decimal.ok_or_else(|| "a finite composition percentage is required".into())
}
