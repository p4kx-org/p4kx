// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Language;
use p4kx_extraction::{SourceUses, SourceValue};
use serde_json::{Value, json};

pub(super) fn present(value: Value) -> Value {
    source(SourceValue::Present(value))
}

pub(super) fn source(value: SourceValue<Value>) -> Value {
    json!(SourceUses::new().value(value, &[], false))
}

pub(super) fn language_sources() -> Value {
    let mut sources = serde_json::Map::new();
    for language in Language::ALL {
        let source = if matches!(language, Language::English | Language::German) {
            present(json!({
                "path": present(json!(language.global_ini_path())),
                "encoding": present(json!("utf8")),
                "lossyDecoding": present(json!(false)),
                "skippedMalformedLines": present(json!(0)),
            }))
        } else {
            source(SourceValue::Absent)
        };
        sources.insert(language.code().to_owned(), source);
    }
    Value::Object(sources)
}
