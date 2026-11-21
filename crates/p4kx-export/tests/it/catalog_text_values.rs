// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::TestResult;

pub(super) fn translation(
    text: &Value,
    language: &str,
    state: &str,
    expected: &Value,
) -> TestResult {
    forms_translation(text, language, state, expected, expected)
}

pub(super) fn forms_translation(
    text: &Value,
    language: &str,
    state: &str,
    source: &Value,
    display: &Value,
) -> TestResult {
    assert_eq!(
        text.pointer("/translations/state"),
        Some(&json!("present")),
        "the language map is present"
    );
    let forms = crate::text_forms_assertions::forms(text, language)?;
    for (field, expected) in [("sourceText", source), ("displayText", display)] {
        let value = forms.get(field).ok_or("required direct text form")?;
        crate::text_forms_assertions::direct(value)?;
        assert_eq!(
            value.get("state"),
            Some(&json!(state)),
            "the {field} state matches the original lookup"
        );
        assert_eq!(
            value.get("value"),
            Some(expected),
            "the {field} payload matches its independently authored expectation"
        );
    }
    Ok(())
}
