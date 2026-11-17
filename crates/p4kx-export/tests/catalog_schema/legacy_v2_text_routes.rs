// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{Result, present_value, resolve, schema_at};

#[derive(Clone, Copy)]
pub(super) enum TextDomain {
    PublicName,
    GameText,
}

#[derive(Clone, Copy)]
pub(super) enum TextPresence {
    Direct,
    Nullable,
}

pub(super) fn check_text_route(
    schema: &Value,
    owner: &str,
    field: &str,
    domain: TextDomain,
    presence: TextPresence,
) -> Result {
    let (registry, reference, protection) = match domain {
        TextDomain::PublicName => ("publicNames", "PublicNameRef", None),
        TextDomain::GameText => ("gameTexts", "GameTextRef", Some(true)),
    };
    let prefix = match presence {
        TextPresence::Direct => "",
        TextPresence::Nullable => "Nullable_",
    };
    let cell = schema_at(schema, owner)?
        .pointer(&format!("/properties/{field}"))
        .ok_or("canonical text field")?;
    assert_eq!(
        cell["$ref"],
        format!("#/$defs/SourcedValueOf{prefix}{reference}"),
        "sourced reference for {owner}/{field}"
    );
    assert_eq!(
        cell.get("x-gameText").and_then(Value::as_bool),
        protection,
        "text policy for {owner}/{field}"
    );
    if matches!(presence, TextPresence::Nullable) {
        check_nullable_reference(resolve(schema, cell)?, reference)?;
    }

    let registry = format!("/texts/{registry}");
    let table = schema_at(schema, &registry)?;
    let expected = format!("#/$defs/CanonicalTextOf{reference}");
    assert_eq!(
        table.pointer("/items/$ref").and_then(Value::as_str),
        Some(expected.as_str()),
        "{owner}/{field} joins its text domain"
    );
    for suffix in [
        "id/value",
        "key/value",
        "translations/value/*/displayText/value",
    ] {
        check_string(schema, &format!("{registry}/[]/{suffix}"))?;
    }
    Ok(())
}

pub(super) fn check_nullable_reference(wrapper: &Value, reference: &str) -> Result {
    let options = present_value(wrapper)
        .and_then(|payload| payload["anyOf"].as_array())
        .ok_or("nullable sourced reference payload")?;
    let path = format!("#/$defs/{reference}");
    assert_eq!(options.len(), 2, "only null or {reference}");
    assert!(
        options.iter().any(|branch| branch["type"] == "null"),
        "present null is a valid payload"
    );
    assert!(
        options.iter().any(|branch| branch["$ref"] == path),
        "present reference uses {path}"
    );
    Ok(())
}

pub(super) fn check_string(schema: &Value, path: &str) -> Result {
    assert_eq!(
        schema_at(schema, path)?.get("type").and_then(Value::as_str),
        Some("string"),
        "string destination {path}"
    );
    Ok(())
}
