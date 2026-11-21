// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use serde_json::Value;

use crate::text_forms_fixture::TestResult;

pub(crate) fn public_is_protected(root: &Value) -> TestResult<bool> {
    let public = root
        .pointer("/$defs/CatalogTexts/properties/publicNames")
        .ok_or("actual public canonical text schema")?;
    marked(root, public, &mut BTreeSet::new())
}

pub(crate) fn forms_pointer(root: &Value) -> TestResult<String> {
    let row = resolve_pointer(root, "/$defs/CatalogTexts/properties/publicNames/items")?;
    let translations = resolve_pointer(root, &format!("{row}/properties/translations"))?;
    let clauses = root
        .pointer(&format!("{translations}/allOf"))
        .and_then(Value::as_array)
        .ok_or("typed translations value rules")?;
    let index = clauses
        .iter()
        .position(|clause| {
            clause
                .pointer("/if/properties/state/const")
                .and_then(Value::as_str)
                == Some("present")
        })
        .ok_or("present translations rule")?;
    let map = resolve_pointer(
        root,
        &format!("{translations}/allOf/{index}/then/properties/value"),
    )?;
    resolve_pointer(root, &format!("{map}/additionalProperties"))
}

fn resolve_pointer(root: &Value, initial: &str) -> TestResult<String> {
    let mut pointer = initial.to_owned();
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(pointer.clone()) {
            return Err("cyclic schema alias".into());
        }
        let node = root
            .pointer(&pointer)
            .ok_or("closed local text schema reference")?;
        let Some(reference) = node.get("$ref").and_then(Value::as_str) else {
            return Ok(pointer);
        };
        reference
            .strip_prefix('#')
            .ok_or("nonlocal text schema reference")?
            .clone_into(&mut pointer);
    }
}

fn marked(root: &Value, node: &Value, seen: &mut BTreeSet<String>) -> TestResult<bool> {
    if node.get("x-gameText").and_then(Value::as_bool) == Some(true) {
        return Ok(true);
    }
    if let Some(reference) = node.get("$ref").and_then(Value::as_str)
        && seen.insert(reference.to_owned())
    {
        let pointer = reference
            .strip_prefix('#')
            .ok_or("nonlocal protected-text reference")?;
        if marked(
            root,
            root.pointer(pointer)
                .ok_or("protected-text reference closure")?,
            seen,
        )? {
            return Ok(true);
        }
    }
    match node {
        Value::Object(object) => {
            for child in object.values() {
                if marked(root, child, seen)? {
                    return Ok(true);
                }
            }
        }
        Value::Array(array) => {
            for child in array {
                if marked(root, child, seen)? {
                    return Ok(true);
                }
            }
        }
        _ => {}
    }
    Ok(false)
}
