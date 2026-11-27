// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::BuiltCatalog;
use p4kx_extraction::Language;
use serde_json::{Value, json};

use crate::text_forms_fixture::{self as fixture, TestResult};

pub(crate) fn forms<'a>(row: &'a Value, language: &str) -> TestResult<&'a Value> {
    let forms = fixture::at(row, &format!("/translations/value/{language}"))?;
    let object = forms
        .as_object()
        .ok_or("required shared TextForms object")?;
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["sourceText", "displayText"]),
        "the language group has exactly two independently sourced forms"
    );
    Ok(forms)
}

pub(crate) fn present(forms: &Value, field: &str, expected: &str) -> TestResult {
    let carrier = fixture::at(forms, &format!("/{field}"))?;
    assert_eq!(
        fixture::at(carrier, "/state")?,
        &json!("present"),
        "a winning source or display string remains present"
    );
    assert_eq!(
        fixture::at(carrier, "/value")?,
        &json!(expected),
        "the selected text form preserves its independently expected characters"
    );
    direct(carrier)?;
    Ok(())
}

pub(crate) fn absent(forms: &Value) -> TestResult {
    for field in ["sourceText", "displayText"] {
        let carrier = fixture::at(forms, &format!("/{field}"))?;
        assert_eq!(
            fixture::at(carrier, "/state")?,
            &json!("absent"),
            "missing and suppressed lookups retain the absent state in both forms"
        );
        assert_eq!(
            fixture::at(carrier, "/value")?,
            &Value::Null,
            "an absent form cannot invent a winning string"
        );
        direct(carrier)?;
    }
    Ok(())
}

pub(crate) fn direct(carrier: &Value) -> TestResult {
    for field in [
        "state",
        "value",
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        assert!(carrier.get(field).is_some(), "required immediate {field}");
    }
    assert!(
        fixture::at(carrier, "/sourceUsesRefs")?.is_array(),
        "immediate form references use the shared array carrier"
    );
    assert_eq!(
        fixture::at(carrier, "/sourceSetComplete")?,
        &json!(true),
        "all original contributors are captured even when encryption is unknown"
    );
    let marker = fixture::at(carrier, "/containsEncryptedSource")?;
    assert!(
        marker.is_null() || marker.is_boolean(),
        "explicit nullable direct marker"
    );
    Ok(())
}

pub(crate) fn summary(
    built: &BuiltCatalog,
    language: Language,
    resolved: usize,
    unresolved: usize,
    placeholders: usize,
) -> TestResult {
    let actual = built
        .summary()
        .texts()
        .iter()
        .find(|row| row.language == language)
        .ok_or("required original language summary")?;
    assert_eq!(actual.resolved, resolved, "display-based occurrence count");
    assert_eq!(
        actual.unresolved, unresolved,
        "empty or missing display count"
    );
    assert_eq!(
        actual.placeholder, placeholders,
        "one count per original placeholder use"
    );
    Ok(())
}
