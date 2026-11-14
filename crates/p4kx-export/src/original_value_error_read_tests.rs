// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::SourceUses;
use serde_json::{Value, json};

use super::super::error_cases::{self, ErrorCase};
use super::super::error_facts::ErrorFacts;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn every_current_error_parameter_survives_without_claiming_original_admission() -> TestResult {
    let sources = SourceUses::new();
    let cases = error_cases::cases();
    assert_eq!(cases.len(), 14, "all current distinct error variants");
    for case in cases {
        let mapped = super::convert(
            &case.error,
            &ErrorFacts {
                site: None,
                sources: &sources,
            },
        )?;
        let mapped = serde_json::to_value(mapped)?;
        assert_eq!(
            mapped.get("type"),
            Some(&json!(case.kind)),
            "exact error tag"
        );
        check_facts(&mapped, &case)?;
    }
    assert!(
        sources.freeze()?.uses().is_empty(),
        "pure mapping minted no source authority"
    );
    Ok(())
}

fn check_facts(mapped: &Value, case: &ErrorCase) -> TestResult {
    let parameters = case
        .parameters
        .as_object()
        .ok_or("parameter expectations")?;
    let controls = case.controls.as_object().ok_or("control expectations")?;
    assert_eq!(
        mapped.as_object().ok_or("typed error")?.len(),
        parameters.len() + controls.len() + 1,
        "every parameter and control, no extra fact"
    );
    for (name, expected) in parameters {
        let cell = mapped.get(name).ok_or("required sourced error fact")?;
        check_parameter(cell, name, expected);
    }
    for (name, expected) in controls {
        assert_eq!(mapped.get(name), Some(expected), "exact control {name}");
    }
    Ok(())
}

fn check_parameter(cell: &Value, name: &str, expected: &Value) {
    assert_eq!(cell.get("state"), Some(&json!("present")), "{name}");
    assert_eq!(cell.get("value"), Some(expected), "exact retained {name}");
    assert_eq!(
        cell.get("sourceUsesRefs"),
        Some(&json!([])),
        "no invented original input"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(false)),
        "no false admission"
    );
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&Value::Null),
        "unknown is preserved"
    );
}
