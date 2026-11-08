// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Map, Value};

/// The canonical domain of a Commodity text reference.
#[derive(Clone, Copy)]
pub(crate) enum TextDomain {
    PublicName,
    GameText,
}

/// Returns the canonical Commodity definition rows.
pub(crate) fn definitions(catalog: &Value) -> Result<&[Value], Box<dyn Error>> {
    catalog
        .pointer("/definitions/commodities/rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "canonical Commodity definitions are missing".into())
}

/// Returns the canonical Commodity presentation rows.
pub(crate) fn presentations(catalog: &Value) -> Result<&[Value], Box<dyn Error>> {
    catalog
        .pointer("/projections/commodityPresentation/rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "canonical Commodity presentations are missing".into())
}

/// Returns the selected Commodity definition with the requested source GUID.
pub(crate) fn definition<'catalog>(
    catalog: &'catalog Value,
    guid: &str,
) -> Result<&'catalog Value, Box<dyn Error>> {
    definitions(catalog)?
        .iter()
        .find(|row| row.pointer("/guid/value").and_then(Value::as_str) == Some(guid))
        .ok_or_else(|| format!("canonical Commodity definition {guid} is missing").into())
}

/// Returns the presentation with the requested Commodity GUID.
pub(crate) fn presentation<'catalog>(
    catalog: &'catalog Value,
    guid: &str,
) -> Result<&'catalog Value, Box<dyn Error>> {
    presentations(catalog)?
        .iter()
        .find(|row| row.pointer("/commodityGuid/value").and_then(Value::as_str) == Some(guid))
        .ok_or_else(|| format!("canonical Commodity presentation {guid} is missing").into())
}

/// Reads every selected Commodity class name without a fallback row.
pub(crate) fn class_names(catalog: &Value) -> Result<Vec<&str>, Box<dyn Error>> {
    definitions(catalog)?
        .iter()
        .map(|row| {
            row.pointer("/className/value")
                .and_then(Value::as_str)
                .ok_or_else(|| "canonical Commodity class name is missing".into())
        })
        .collect()
}

/// Checks one reference against its actual canonical text row and language values.
pub(crate) fn assert_text(
    catalog: &Value,
    reference: &Value,
    domain: TextDomain,
    key: Option<&str>,
    english: Option<&str>,
    german: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let reference = reference
        .get("value")
        .ok_or("sourced text reference is missing")?;
    if reference.is_null() {
        assert_eq!(
            (key, english, german),
            (None, None, None),
            "a null reference cannot supply a text key or translation"
        );
        return Ok(());
    }
    let reference = reference.as_str().ok_or("text reference is not a string")?;
    let domain = match domain {
        TextDomain::PublicName => "publicNames",
        TextDomain::GameText => "gameTexts",
    };
    let rows = catalog
        .get("texts")
        .and_then(|texts| texts.get(domain))
        .and_then(Value::as_array)
        .ok_or("canonical text domain is missing")?;
    let row = rows
        .iter()
        .find(|row| row.pointer("/id/value").and_then(Value::as_str) == Some(reference))
        .ok_or("canonical text reference is unresolved")?;
    assert_eq!(
        row.pointer("/key/value")
            .ok_or("canonical text key payload is missing")?
            .as_str(),
        key,
        "the selected text row retains the exact authored key"
    );
    let translations = row
        .pointer("/translations/value")
        .and_then(Value::as_object)
        .ok_or("canonical text translations are missing")?;
    assert_eq!(
        translated(translations, "en")?,
        english,
        "the selected English translation keeps absence distinct from an empty string"
    );
    assert_eq!(
        translated(translations, "de")?,
        german,
        "the selected German translation keeps absence distinct from an empty string"
    );
    Ok(())
}

fn translated<'row>(
    translations: &'row Map<String, Value>,
    language: &str,
) -> Result<Option<&'row str>, Box<dyn Error>> {
    let Some(value) = translations.get(language) else {
        return Ok(None);
    };
    match value.get("state").and_then(Value::as_str) {
        Some("present") => value
            .get("value")
            .and_then(Value::as_str)
            .map(Some)
            .ok_or_else(|| "canonical translation is not a sourced string".into()),
        Some("absent") if value.get("value").is_some_and(Value::is_null) => Ok(None),
        _ => Err("canonical translation state is invalid".into()),
    }
}
