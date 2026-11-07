// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Map, Value};

use crate::MediaError;

pub(crate) fn insert_payload(
    base: &mut Value,
    name: &str,
    mut payload: Value,
) -> Result<(), MediaError> {
    let payload_root = payload
        .as_object_mut()
        .ok_or_else(|| contract("payload schema is not an object"))?;
    let definitions = payload_root.remove("$defs");
    payload_root.remove("$schema");
    let root = base
        .as_object_mut()
        .ok_or_else(|| contract("media record schema is not an object"))?;
    let target = root
        .entry("$defs")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| contract("media record definitions are not an object"))?;
    if let Some(definitions) = definitions {
        let Value::Object(definitions) = definitions else {
            return Err(contract("payload definitions are not an object"));
        };
        for (name, schema) in definitions {
            insert_definition(target, &name, schema)?;
        }
    }
    insert_definition(target, name, payload)
}

fn insert_definition(
    target: &mut Map<String, Value>,
    name: &str,
    value: Value,
) -> Result<(), MediaError> {
    if !value.is_object() && !value.is_boolean() {
        return Err(contract("schema definition is not an object or boolean"));
    }
    if let Some(existing) = target.get(name) {
        if existing != &value {
            return Err(contract(
                "schema definition collides with a different definition",
            ));
        }
        return Ok(());
    }
    target.insert(name.to_owned(), value);
    Ok(())
}

fn contract(detail: &str) -> MediaError {
    MediaError::Contract(detail.into())
}
