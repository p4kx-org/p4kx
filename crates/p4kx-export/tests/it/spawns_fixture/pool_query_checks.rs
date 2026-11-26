// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use serde::Serialize;
use serde_json::Value;

pub(super) fn registered_pool_rows<'catalog>(
    catalog: &'catalog Value,
    contexts: &[Value],
    path: &str,
    kind: &str,
) -> Result<Vec<&'catalog Value>, Box<dyn Error>> {
    let member = if kind == "location" { "query" } else { "pool" };
    let mut ids = BTreeSet::new();
    for context in contexts {
        let registrations = context["spawnPools"]
            .as_array()
            .ok_or("producer pool registrations")?;
        for registration in registrations {
            let result = &registration["result"];
            if result["type"].as_str() == Some(kind) && result["empty"] != true {
                let id = result[member]["value"]
                    .as_str()
                    .ok_or("producer pool GUID")?;
                ids.insert(id);
            }
        }
    }
    let rows = catalog
        .pointer(path)
        .and_then(Value::as_array)
        .ok_or("producer pool rows")?;
    Ok(rows
        .iter()
        .filter(|row| row["guid"].as_str().is_some_and(|id| ids.contains(id)))
        .collect())
}

pub(super) fn check_game_text(
    catalog: &Value,
    actual: &impl Serialize,
) -> Result<(), Box<dyn Error>> {
    let encoded = serde_json::to_value(actual)?;
    let id = encoded
        .pointer("/id/value")
        .and_then(Value::as_str)
        .ok_or("SDK protected name ID")?;
    let catalog_texts = catalog["texts"]["gameTexts"]
        .as_array()
        .ok_or("producer game texts")?;
    let producer = catalog_texts
        .iter()
        .find(|row| row.pointer("/id/value").and_then(Value::as_str) == Some(id))
        .ok_or("matching producer protected name")?;
    assert_eq!(
        encoded.pointer("/key/value"),
        producer.pointer("/key/value")
    );
    let local = encoded
        .pointer("/translations/value")
        .and_then(Value::as_object)
        .ok_or("SDK protected name languages")?;
    let expected = producer
        .pointer("/translations/value")
        .and_then(Value::as_object)
        .ok_or("producer protected name languages")?;
    assert_eq!(local.len(), expected.len());
    for (language, forms) in local {
        let expected_forms = expected.get(language).ok_or("producer locale")?;
        for path in ["/sourceText/value", "/displayText/value"] {
            assert_eq!(forms.pointer(path), expected_forms.pointer(path));
        }
    }
    Ok(())
}
