// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::Location;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn opaque_reader_preserves_declared_capture_limit_contract() -> TestResult {
    let location: Location = serde_json::from_value(json!({
        "type": "opaque", "reason": "captureLimit"
    }))?;
    location.require_shape()?;
    Ok(())
}

#[test]
fn opaque_reader_admits_unavailable_original_coordinate() -> TestResult {
    let location: Location = serde_json::from_value(json!({
        "type": "opaque", "reason": "unavailable"
    }))?;
    location.require_shape()?;
    Ok(())
}

#[test]
fn opaque_reader_rejects_missing_and_unknown_reason() {
    for value in [
        json!({"type": "opaque"}),
        json!({"type": "opaque", "reason": "invented"}),
    ] {
        let accepted = serde_json::from_value::<Location>(value)
            .is_ok_and(|location| location.require_shape().is_ok());
        assert!(
            !accepted,
            "missing or unknown opaque reason cannot certify an admitted coordinate"
        );
    }
}
