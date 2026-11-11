// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

pub(crate) type Checked<T = ()> = Result<T, Box<dyn Error>>;

/// Strict test navigation over one serialized owner-local Loot value inventory.
pub(crate) struct Graph<'catalog> {
    row: &'catalog Value,
    values: &'catalog [Value],
}

impl<'catalog> Graph<'catalog> {
    pub(crate) fn new(catalog: &'catalog Value, wanted: &str) -> Checked<Self> {
        let table = catalog
            .pointer("/definitions/lootPoolFilters")
            .ok_or("canonical Loot table")?;
        assert_eq!(
            table.get("availability"),
            Some(&json!("available")),
            "the declared original family is available"
        );
        let rows = array(table, "rows")?;
        let mut found = None;
        for row in rows {
            let guid = present(row.get("guid").ok_or("sourced physical row GUID")?)?
                .as_str()
                .ok_or("GUID text")?;
            if guid == wanted {
                if found.is_some() {
                    return Err("ambiguous nominal Loot row identity".into());
                }
                found = Some(row);
            }
        }
        let row = found.ok_or("requested original Loot row")?;
        Ok(Self {
            row,
            values: array(row, "values")?,
        })
    }

    pub(crate) fn root(&self, name: &str) -> Checked<&'catalog Value> {
        self.field(array(self.row, "fields")?, name)
    }

    pub(crate) fn field(&self, fields: &[Value], name: &str) -> Checked<&'catalog Value> {
        let mut found = None;
        for field in fields {
            let actual = present(field.get("name").ok_or("sourced declaration name")?)?
                .as_str()
                .ok_or("original name text")?;
            if actual == name {
                if found.is_some() {
                    return Err("ambiguous same-name source declarations".into());
                }
                found = Some(field);
            }
        }
        self.follow(
            found
                .ok_or_else(|| format!("original {name} declaration"))?
                .get("value")
                .ok_or("source value reference")?,
        )
    }

    pub(crate) fn follow(&self, reference: &Value) -> Checked<&'catalog Value> {
        let text = reference
            .get("valueIndex")
            .and_then(Value::as_str)
            .ok_or("DecimalU64 local reference")?;
        let index: u64 = text.parse()?;
        if text != index.to_string() {
            return Err("noncanonical local reference".into());
        }
        self.values
            .get(usize::try_from(index)?)
            .ok_or_else(|| "local reference outside this original row".into())
    }
}

pub(crate) fn array<'value>(owner: &'value Value, name: &str) -> Checked<&'value [Value]> {
    owner
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("required {name} array").into())
}

pub(crate) fn present(cell: &Value) -> Checked<&Value> {
    assert_eq!(
        cell.get("state"),
        Some(&json!("present")),
        "exact immediate source state"
    );
    for name in [
        "value",
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        assert!(
            cell.get(name).is_some(),
            "required immediate sourced member {name}"
        );
    }
    cell.get("value")
        .ok_or_else(|| "required present source value".into())
}

pub(crate) fn payload<'node>(node: &'node Value, kind: &str) -> Checked<&'node Value> {
    assert_eq!(
        node.get("originalError"),
        Some(&Value::Null),
        "readable source has no original decoder error"
    );
    let payload = present(node.get("value").ok_or("immediate original datum")?)?;
    assert_eq!(
        payload.get("type"),
        Some(&json!(kind)),
        "exact original value variant"
    );
    Ok(payload)
}

pub(crate) fn public_text(node: &Value) -> Checked<&str> {
    let text = payload(node, "string")?
        .get("value")
        .ok_or("typed original string")?;
    assert_eq!(
        text.get("type"),
        Some(&json!("public")),
        "known designer text is public"
    );
    text.get("sourceText")
        .and_then(Value::as_str)
        .ok_or_else(|| "complete original string".into())
}

pub(crate) fn reference_guid(node: &Value) -> Checked<&str> {
    present(
        payload(node, "recordReference")?
            .pointer("/value/guid")
            .ok_or("independent authored GUID cell")?,
    )?
    .as_str()
    .ok_or_else(|| "original GUID text".into())
}

pub(crate) fn source_null(node: &Value) {
    assert_eq!(
        node.pointer("/value/state"),
        Some(&json!("null")),
        "source null remains distinct from absence/unreadability"
    );
    assert_eq!(
        node.pointer("/value/value"),
        Some(&Value::Null),
        "explicit original null payload"
    );
    assert_eq!(
        node.get("originalError"),
        Some(&Value::Null),
        "a null pointer is not a decoder error"
    );
}

pub(crate) fn structure<'node>(node: &'node Value, expected: &str) -> Checked<&'node [Value]> {
    let payload = payload(node, "struct")?;
    assert_eq!(
        present(payload.get("sourceType").ok_or("actual nested type")?)?,
        &json!(expected),
        "exact original concrete type"
    );
    array(payload, "fields")
}

pub(crate) fn elements(node: &Value) -> Checked<&[Value]> {
    array(payload(node, "array")?, "elements")
}
