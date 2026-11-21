// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Deserialize;
use serde_json::Value;

use super::TestResult;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportedValue<T> {
    state: String,
    value: Option<T>,
    contains_encrypted_source: Option<bool>,
    source_uses_refs: Vec<String>,
    source_set_complete: bool,
}

impl<T> ImportedValue<T> {
    fn public_value(&self) -> Option<&T> {
        (self.state == "present"
            && self.source_set_complete
            && self.contains_encrypted_source == Some(false))
        .then_some(self.value.as_ref())
        .flatten()
    }
}

pub(super) fn assert_classification(catalog: &Value) -> TestResult {
    let guid: ImportedValue<String> = serde_json::from_value(
        super::row(catalog)?
            .get("guid")
            .ok_or("canonical identity carrier")?
            .clone(),
    )?;
    assert!(
        guid.public_value().is_some(),
        "the DCB payload is unencrypted"
    );
    assert!(
        !guid.source_uses_refs.is_empty(),
        "immediate origin refers to actual uses"
    );
    let names = super::text_row(catalog, "publicNames", "@fixture_name")?;
    let translations = names
        .pointer("/translations/value")
        .ok_or("canonical language map")?;
    for field in ["sourceText", "displayText"] {
        assert_form(translations, field)?;
    }
    Ok(())
}

fn assert_form(translations: &Value, field: &str) -> TestResult {
    let english = imported(translations, "en", field)?;
    let german = imported(translations, "de", field)?;
    let french = imported(translations, "fr", field)?;
    assert_eq!(
        english.value.as_deref(),
        Some("English winner"),
        "each English form retains the actual original winner"
    );
    assert_eq!(
        english.contains_encrypted_source,
        Some(true),
        "the actual encrypted locale contributes to each form"
    );
    assert!(
        english.public_value().is_none(),
        "encrypted language contribution is not public"
    );
    assert!(
        german.public_value().is_some(),
        "this language uses only known unencrypted input"
    );
    assert_eq!(
        french.contains_encrypted_source, None,
        "unknown encryption remains explicit per form"
    );
    assert!(
        french.public_value().is_none(),
        "unknown origin cannot be promoted by read success"
    );
    assert!(
        !english.source_uses_refs.is_empty() && !german.source_uses_refs.is_empty(),
        "each localized value carries its own uses"
    );
    Ok(())
}

fn imported(
    translations: &Value,
    language: &str,
    field: &str,
) -> TestResult<ImportedValue<String>> {
    let carrier = translations
        .get(language)
        .and_then(|forms| forms.get(field))
        .ok_or("actual direct language form")?;
    crate::text_forms_assertions::direct(carrier)?;
    Ok(serde_json::from_value(carrier.clone())?)
}
