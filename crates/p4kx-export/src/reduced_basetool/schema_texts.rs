// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValueState;
use schemars::Schema;
use serde_json::{Value, json};

use super::super::plan;
use super::closed_object;

pub(super) fn schema(state: &Schema) -> Value {
    closed_object([
        ("publicNames", names(state, TextDomain::Public)),
        ("gameTexts", names(state, TextDomain::Protected)),
    ])
}

#[derive(Clone, Copy)]
enum TextDomain {
    Public,
    Protected,
}

fn names(state: &Schema, domain: TextDomain) -> Value {
    let locales = plan::LOCALES
        .into_iter()
        .map(|locale| (locale.code().to_owned(), forms(state, domain)));
    let translations = json!({
        "type": "object",
        "properties": locales.collect::<serde_json::Map<_, _>>(),
        "additionalProperties": false,
    });
    let row = closed_object([
        ("id", present_cell(domain.text())),
        ("key", present_cell(domain.text())),
        ("translations", present_cell(translations)),
    ]);
    json!({"type": "array", "items": row})
}

fn forms(state: &Schema, domain: TextDomain) -> Value {
    closed_object([
        ("sourceText", localized_cell(state, domain)),
        ("displayText", localized_cell(state, domain)),
    ])
}

fn present_cell(value: Value) -> Value {
    marker(json!({"const": SourceValueState::Present}), value)
}

fn localized_cell(state: &Schema, domain: TextDomain) -> Value {
    json!({
        "allOf": [
            marker(json!(state), json!({"anyOf": [domain.text(), {"type": "null"}]})),
            {
                "if": {"properties": {"state": {"const": SourceValueState::Present}}},
                "then": {"properties": {"value": {"type": "string"}}},
                "else": {"properties": {"value": {"type": "null"}}}
            }
        ]
    })
}

fn marker(state: Value, value: Value) -> Value {
    closed_object([
        ("state", state),
        ("value", value),
        (
            "containsEncryptedSource",
            json!({"type": ["boolean", "null"]}),
        ),
    ])
}

impl TextDomain {
    fn text(self) -> Value {
        match self {
            Self::Public => json!({"type": "string"}),
            Self::Protected => json!({"type": "string", "x-gameText": true}),
        }
    }
}
